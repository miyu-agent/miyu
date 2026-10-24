//! 机主私人的脚本和 MCP 服务（施工 U23 中上）：机主家目录里放的脚本、个人设置里的 MCP 服务只给机主这一侧的会话；成员的会话
//! 工具面上没有，预设的功能里也不列；机主的照旧有。这个漏洞从 Y-2 下下（成员能开会话）起一直在，U23 上下以后堵上。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::scripts::{core, echo};
use crate::support::login::member;
use crate::support::mcp::{SERVER, until_listed};
use crate::support::*;

/// 新会话会拿到的工具名（`session.tools` 不写会话）。
async fn face(client: &mut Client) -> Vec<String> {
    let reply = client.call("t", "session.tools", json!({})).await;
    reply["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_string))
        .collect()
}

/// 工具面上有没有机主私人的那两样：脚本 `weather`、服务 `probe` 的工具。
fn private(face: &[String]) -> (bool, bool) {
    (
        face.iter().any(|name| name == "weather"),
        face.iter().any(|name| name.starts_with("mcp__probe__")),
    )
}

/// 预设 `full` 列出来的功能编号。
async fn features(client: &mut Client) -> Vec<Value> {
    let reply = client
        .call("p", "preset.get", json!({"preset": "full"}))
        .await;
    reply["result"]["features"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|one| one["id"].clone())
        .collect()
}

#[tokio::test]
async fn the_owner_s_own_scripts_and_mcp_servers_stay_out_of_a_member_s_sessions() {
    let home = Home::new();
    home.write("home/alice/scripts/weather.py", &echo("weather"));
    home.write(
        "home/alice/settings.toml",
        &format!(
            "[mcp.probe]\ncommand = ['{SERVER}', 'kit']\ntimeout_ms = 1000\nneed_load = false\n\n[mcp.words]\ncommand = ['{SERVER}', 'prompts']\ntimeout_ms = 1000\n"
        ),
    );
    let core = core(&home, &Script::new([]));
    let mut alice = Client::connect(Arc::clone(&core));
    alice.hello().await;
    until_listed(&mut alice, "probe", |one| one["state"] == "running").await;
    until_listed(&mut alice, "words", |one| one["state"] == "running").await;
    let mut bob = member(&core, "bob").await;
    // 先断言真有：机主的会话照旧有，不然下面的「没有」白过。
    assert_eq!(
        private(&face(&mut alice).await),
        (true, true),
        "机主的照旧有"
    );
    assert_eq!(private(&face(&mut bob).await), (false, false), "成员的没有");
    let listed = features(&mut alice).await;
    assert!(listed.contains(&json!("script-weather")), "{listed:?}");
    let listed = features(&mut bob).await;
    assert!(
        !listed
            .iter()
            .any(|id| id == "script-weather" || id.as_str().is_some_and(|id| id.contains("probe"))),
        "成员的预设不列机主私人的：{listed:?}"
    );
    // 服务的提示词当斜杠命令：不写会话问菜单的，成员也看不到机主私人服务的。
    assert!(
        menu(&mut alice).await.contains(&"words:hello".to_string()),
        "机主的照旧有"
    );
    assert!(
        !menu(&mut bob).await.contains(&"words:hello".to_string()),
        "成员的没有"
    );
    // 敲了也当没有：成员在自己的会话里跑不了机主私人服务的提示词。
    let bob_id = miyu_kernel::id::AccountId::parse("bob").expect("合写法");
    let workspace = home.root.workspace(&bob_id);
    std::fs::create_dir_all(&workspace).expect("建得了");
    let session = bob.create("c1", &workspace.to_string_lossy()).await;
    assert!(
        !menu_of(&mut bob, Some(&session))
            .await
            .contains(&"words:hello".to_string()),
        "写了会话的菜单也没有"
    );
    let ran = bob
        .call(
            "r",
            "command.run",
            json!({"session": session, "text": "/words:hello"}),
        )
        .await;
    assert_eq!(
        ran["error"]["data"]["reason"], "unknown_command",
        "当没有：{ran}"
    );
    core.stop_extensions().await;
}

/// 不写会话的命令菜单里的名字。
async fn menu(client: &mut Client) -> Vec<String> {
    menu_of(client, None).await
}

/// 命令菜单里的名字，写了会话 `session` 的问那个会话。
async fn menu_of(client: &mut Client, session: Option<&str>) -> Vec<String> {
    let params = session.map_or_else(|| json!({}), |session| json!({"session": session}));
    let reply = client.call("c", "command.catalog", params).await;
    reply["result"]["commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|one| one["name"].as_str().map(str::to_string))
        .collect()
}

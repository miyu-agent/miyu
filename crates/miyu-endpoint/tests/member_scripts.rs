//! 成员自己的脚本（施工 U23 中下）：成员 bob 家目录 `scripts/` 里的、他自己那一层包带的，进他自己的工具面，照他的会话跑；和机主的
//! 同名的，他的会话里是他的那个；机主的会话没有他的。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::id::AccountId;
use miyu_session::testkit::{Play, Script};

use crate::scripts::{core, echo};
use crate::support::extensions::{dir, until_turns_in};
use crate::support::login::member;
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

/// 会话日志里工具结果的字。
fn results(home: &Home, account: &AccountId, session: &str) -> Vec<String> {
    miyu_store::log::read_events(&dir(home, account, session))
        .expect("读得了")
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_member_s_own_scripts_are_his_alone() {
    let home = Home::new();
    home.write("home/alice/scripts/weather.py", &echo("weather"));
    let script = Arc::new(Script::new([
        Play::calls(&[("weather", &json!({"say": "hi"}).to_string())]),
        Play::Says("好。"),
    ]));
    let core = core(&home, &script);
    let mut bob = member(&core, "bob").await;
    // 注册以后才放（家目录先在的话用户名就算占了）。
    home.write(
        "home/bob/scripts/weather.py",
        &echo("bob-weather").replace("# 显示名称：回声\n", "# Display name: Bob weather\n"),
    );
    home.write("home/bob/scripts/notes.py", &echo("notes"));
    home.write(
        "home/bob/packages/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n",
    );
    home.write("home/bob/packages/kit/scripts/dice.py", &echo("dice"));
    let bob_id = AccountId::parse("bob").expect("合写法");
    let workspace = home.root.workspace(&bob_id);
    std::fs::create_dir_all(&workspace).expect("建得了");
    let session = bob.create("c1", &workspace.to_string_lossy()).await;
    bob.say("s1", &session, "天气").await;
    until_turns_in(&dir(&home, &bob_id, &session), 1).await;
    let sent: Vec<String> = script.requests()[0]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    for name in ["weather", "notes", "dice"] {
        assert!(
            sent.contains(&name.to_string()),
            "他自己的 {name} 在他的工具面上：{sent:?}"
        );
    }
    // 同名的是他自己的那个：来处照他的（显示名是他写的）；跑起来照他的会话的沙盒（没有能关住读的沙盒的机器上拒，MB-1），绝不会
    // 跑成机主的那个。
    let tools = bob
        .call("t", "session.tools", json!({"session": session}))
        .await;
    let weather = tools["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "weather"))
        .cloned()
        .unwrap_or_else(|| panic!("{tools}"));
    assert_eq!(weather["source"]["name"], "Bob weather", "{weather}");
    let said = results(&home, &bob_id, &session);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(!said[0].starts_with("weather:"), "不会跑成机主的：{said:?}");
    let mut alice = Client::connect(Arc::clone(&core));
    alice.hello().await;
    let owner: Vec<String> = face(&mut alice).await;
    assert!(
        owner.contains(&"weather".to_string()),
        "机主的照旧有她自己的"
    );
    assert!(
        !owner.iter().any(|name| name == "notes" || name == "dice"),
        "机主的会话没有成员的：{owner:?}"
    );
    let listed: Vec<Value> = bob.call("p", "preset.get", json!({"preset": "full"})).await["result"]
        ["features"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        listed.iter().any(|one| one["id"] == "script-notes"),
        "他的预设页列他自己的：{listed:?}"
    );
    // 他的 `scripts/` 变了：看着的那一路重找他那一层。
    home.write("home/bob/scripts/late.py", &echo("late"));
    until_on_face(&mut bob, "late").await;
    // 他那一层的包变了、核心重找脚本（机主装了别的包）：他那一层跟着重找。
    home.write(
        "home/bob/packages/more/package.toml",
        "[package]\nname = { en = \"More\" }\n",
    );
    home.write("home/bob/packages/more/scripts/coin.py", &echo("coin"));
    let folder = home.work.join("other");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Other\" }\n",
    )
    .expect("写得进");
    let installed = alice
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "other", "{installed}");
    until_on_face(&mut bob, "coin").await;
    assert!(
        !face(&mut alice).await.contains(&"coin".to_string()),
        "机主的没有"
    );
}

/// 等到新会话会拿到的工具里有 `name`（最多 20 秒）。
async fn until_on_face(client: &mut Client, name: &str) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let now = face(client).await;
        if now.iter().any(|one| one == name) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {name}：{now:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

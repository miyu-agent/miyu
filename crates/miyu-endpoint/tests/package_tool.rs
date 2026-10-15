//! 她装、卸软件包（施工 F-10 下，`docs/blueprint/tools/package.md`，设计 `31-软件包.md` 第六节）：真核心的会话调 `package`。
//! 工作区那一级每次问人，确认请求的说明里带着看一眼的那一份（装的同 `package.install` 带 `preview`，卸的同 `package.remove`
//! 带 `preview`）；允许了才装、卸。完全放开直接做、不问。清单写错的不问，当场照拒绝交回她。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 工作目录里放一个只带技能的包 `id`，交回它的文件夹。
fn kit(home: &Home, id: &str) -> PathBuf {
    let folder = home.work.join(id);
    std::fs::create_dir_all(folder.join("skills/notes")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Kit\" }\n",
    )
    .expect("写得进");
    std::fs::write(
        folder.join("skills/notes/SKILL.md"),
        "---\nname: notes\ndescription: Take notes.\n---\nbody\n",
    )
    .expect("写得进");
    folder
}

/// 真核心，工具目录是基础系统；在工作区里造会话，`full` 的切到完全放开，说一句。
async fn started(home: &Home, plays: Vec<Play>, full: bool) -> (Client, String) {
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let core = Arc::new(home.core_full(&Script::new(plays), tools, None, TOKEN));
    let mut client = Client::connect(core);
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    if full {
        let params = json!({"session": session, "level": "full"});
        let reply = client
            .call("p1", "session.set_permission_level", params)
            .await;
        assert_eq!(reply["result"], json!({}), "{reply}");
    }
    client.say("s1", &session, "装上它").await;
    (client, session)
}

/// 会话里每一条工具结果的字，照先后。
fn results(home: &Home, session: &str) -> Vec<String> {
    home.log(session)
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

/// 日志里的确认请求，照先后。
fn requests(home: &Home, session: &str) -> Vec<Value> {
    logged(home, session)
        .into_iter()
        .filter(|event| event["kind"] == json!("tool.approval_requested"))
        .map(|event| event["body"].clone())
        .collect()
}

fn installed(home: &Home, id: &str) -> bool {
    home.root
        .path()
        .join("home/alice/packages")
        .join(id)
        .exists()
}

#[tokio::test]
async fn at_the_workspace_level_each_change_asks_with_the_preview() {
    let home = Home::new();
    let folder = kit(&home, "kit");
    let plays = vec![
        Play::calls(&[("package", &json!({"install": folder}).to_string())]),
        Play::calls(&[("package", r#"{"remove":"kit"}"#)]),
        Play::Says("好了。"),
    ];
    let (mut client, session) = started(&home, plays, false).await;
    until("装的确认请求", || {
        requests(&home, &session).len() == 1
    })
    .await;
    let asked = requests(&home, &session).remove(0);
    assert_eq!(asked["access"], "install", "{asked}");
    assert!(asked.get("rule").is_none(), "不提规则：{asked}");
    let preview = &asked["detail"]["preview"];
    assert_eq!(
        (preview["package"].clone(), preview["skills"].clone()),
        (json!("kit"), json!(["notes"])),
        "卡片照看一眼的那一份：{asked}"
    );
    assert!(!installed(&home, "kit"), "还没回答，没装");
    let call = asked["call_id"].as_str().expect("有编号").to_string();
    let reply = client
        .call(
            "a1",
            "session.answer",
            json!({"session": session, "call": call, "decision": "once"}),
        )
        .await;
    assert!(reply["result"].is_object(), "{reply}");
    until("卸的确认请求", || {
        requests(&home, &session).len() == 2
    })
    .await;
    assert!(installed(&home, "kit"), "允许了才装");
    let asked = requests(&home, &session).remove(1);
    assert_eq!(asked["detail"]["preview"]["package"], "kit", "{asked}");
    assert_eq!(asked["detail"]["preview"]["layer"], "home", "{asked}");
    let call = asked["call_id"].as_str().expect("有编号").to_string();
    client
        .call(
            "a2",
            "session.answer",
            json!({"session": session, "call": call, "decision": "once"}),
        )
        .await;
    home.until_turns(&session, 1).await;
    assert!(!installed(&home, "kit"), "允许了才卸");
    assert_eq!(
        results(&home, &session),
        [
            "Installed kit. Its tools and skills come in on the next turn.\n",
            "Removed kit.\n"
        ]
    );
}

#[tokio::test]
async fn full_access_installs_without_asking() {
    let home = Home::new();
    let folder = kit(&home, "kit");
    let plays = vec![
        Play::calls(&[("package", &json!({"install": folder}).to_string())]),
        Play::Says("好了。"),
    ];
    let (_client, session) = started(&home, plays, true).await;
    home.until_turns(&session, 1).await;
    assert!(requests(&home, &session).is_empty(), "不问");
    assert!(installed(&home, "kit"));
}

#[tokio::test]
async fn a_folder_that_is_not_a_package_is_refused_without_asking() {
    let home = Home::new();
    let folder = home.work.join("broken");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), "[package\n").expect("写得进");
    let plays = vec![
        Play::calls(&[("package", &json!({"install": folder}).to_string())]),
        Play::Says("写错了。"),
    ];
    let (_client, session) = started(&home, plays, false).await;
    home.until_turns(&session, 1).await;
    assert!(requests(&home, &session).is_empty(), "看不了的不问");
    let said = results(&home, &session);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("is not a valid package, line 1"),
        "{said:?}"
    );
    assert!(!installed(&home, "broken"));
}

/// 和装着的包撞了名（施工 F-8 六补修）：核心拒 `package_conflict`，工具结果一处一行说撞了什么、在哪个包里，她照它改。
#[tokio::test]
async fn a_name_clash_tells_her_what_and_where() {
    let home = Home::new();
    home.write(
        "home/alice/packages/zkit/package.toml",
        "[package]\nname = { en = \"Z\" }\n",
    );
    home.write(
        "home/alice/packages/zkit/skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Take notes.\n---\nbody\n",
    );
    let folder = kit(&home, "akit");
    let plays = vec![
        Play::calls(&[("package", &json!({"install": folder}).to_string())]),
        Play::Says("撞了。"),
    ];
    let (_client, session) = started(&home, plays, true).await;
    home.until_turns(&session, 1).await;
    let said = results(&home, &session);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("cannot be installed: installed packages already use these names.\n- skill notes (in zkit)\n"),
        "{said:?}"
    );
    assert!(!installed(&home, "akit"));
}

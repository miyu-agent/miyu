//! 她装、卸软件包（施工 F-10 下，`docs/blueprint/tools/package.md`，设计 `31-软件包.md` 第六节）：真核心的会话调 `package`；
//! 施工 BS-1 中起它不上工具面，她跑的命令里的 `miyu pkg` 经通道调（`support::pkg`），确认请求带着那一次的序号。
//! 工作区那一级每次问人，确认请求的说明里带着看一眼的那一份（装的同 `package.install` 带 `preview`，卸的同 `package.remove`
//! 带 `preview`）；允许了才装、卸。完全放开直接做、不问。清单写错的不问，当场照拒绝交回她。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};

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

/// 真核心，工具目录是基础系统；在工作区里造会话，`full` 的切到完全放开，说一句。她跑一条命令，里面的 `miyu pkg` 照先后调
/// `commands`（`package` 的参数原文），然后说「好了」。
async fn started(home: &Home, commands: Vec<String>, full: bool) -> (Client, String) {
    let commands: Vec<(&str, &str)> = commands
        .iter()
        .map(|args| ("package", args.as_str()))
        .collect();
    let plays = vec![
        Play::calls(&[("shell", &pkg::run(&commands))]),
        Play::Says("好了。"),
    ];
    let core = Arc::new(home.core_full(&Script::new(plays), pkg::catalog(), None, TOKEN));
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
    home.root.path().join("system/packages").join(id).exists()
}

#[tokio::test]
async fn at_the_workspace_level_each_change_asks_with_the_preview() {
    let home = Home::new();
    let folder = kit(&home, "kit");
    let commands = vec![
        json!({"install": folder}).to_string(),
        r#"{"remove":"kit"}"#.to_string(),
    ];
    let (mut client, session) = started(&home, commands, false).await;
    until("装的确认请求", || {
        requests(&home, &session).len() == 1
    })
    .await;
    let asked = requests(&home, &session).remove(0);
    assert_eq!(asked["access"], "install", "{asked}");
    assert!(asked.get("rule").is_none(), "不提规则：{asked}");
    assert_eq!(asked["nested"], 1, "命令里的第一次：{asked}");
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
            json!({"session": session, "call": call, "decision": "once", "nested": 1}),
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
            json!({"session": session, "call": call, "decision": "once", "nested": 2}),
        )
        .await;
    home.until_turns(&session, 1).await;
    assert!(!installed(&home, "kit"), "允许了才卸");
    assert_eq!(
        results(&home, &session),
        [
            "package ok text=Installed kit. Its tools and skills come in on the next turn.\n\npackage ok text=Removed kit.\n\nheld true,false,true,false"
        ]
    );
}

#[tokio::test]
async fn full_access_installs_without_asking() {
    let home = Home::new();
    let folder = kit(&home, "kit");
    let commands = vec![json!({"install": folder}).to_string()];
    let (_client, session) = started(&home, commands, true).await;
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
    let commands = vec![json!({"install": folder}).to_string()];
    let (_client, session) = started(&home, commands, false).await;
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
        "system/packages/zkit/package.toml",
        "[package]\nname = { en = \"Z\" }\n",
    );
    home.write(
        "system/packages/zkit/skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Take notes.\n---\nbody\n",
    );
    let folder = kit(&home, "akit");
    let commands = vec![json!({"install": folder}).to_string()];
    let (_client, session) = started(&home, commands, true).await;
    home.until_turns(&session, 1).await;
    let said = results(&home, &session);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains(
            "cannot be installed: these names are already in use.\n- skill notes (in zkit)\n"
        ),
        "{said:?}"
    );
    assert!(!installed(&home, "akit"));
}

/// 她装回卸掉的出厂包（施工 BS-1 中下修，网页报的）：命令里的 `miyu pkg install mermaid` 不走软件源，问人的卡片是装回的那一份
/// （`restores`），允许了装回来；写组名的整组装回。
#[tokio::test]
async fn she_brings_back_a_removed_shipped_package_and_a_group() {
    let home = Home::new();
    let commands: Vec<(&str, &str)> = vec![
        ("package", r#"{"install":"mermaid"}"#),
        ("package", r#"{"install":"basesystem"}"#),
    ];
    let plays = vec![
        Play::calls(&[("shell", &pkg::run(&commands))]),
        Play::Says("装回来了。"),
    ];
    let core = Arc::new(home.core_full(&Script::new(plays), pkg::catalog(), None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    for id in ["mermaid", "trash"] {
        let removed = client
            .call("r", "package.remove", json!({"package": id}))
            .await;
        assert!(removed["result"].is_object(), "{removed}");
    }
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    client.say("s1", &session, "把 mermaid 装回来").await;
    until("装回的确认请求", || {
        requests(&home, &session).len() == 1
    })
    .await;
    let asked = requests(&home, &session).remove(0);
    assert_eq!(
        asked["detail"]["preview"]["restores"], true,
        "卡片是装回的那一份：{asked}"
    );
    assert_eq!(asked["detail"]["preview"]["package"], "mermaid", "{asked}");
    let call = asked["call_id"].as_str().expect("有编号").to_string();
    let reply = client
        .call(
            "a1",
            "session.answer",
            json!({"session": session, "call": call, "decision": "once", "nested": 1}),
        )
        .await;
    assert!(reply["result"].is_object(), "{reply}");
    until("整组的确认请求", || {
        requests(&home, &session).len() == 2
    })
    .await;
    let asked = requests(&home, &session).remove(1);
    assert_eq!(asked["detail"]["preview"]["group"], "basesystem", "{asked}");
    assert_eq!(
        asked["detail"]["preview"]["packages"],
        json!(["trash"]),
        "{asked}"
    );
    client
        .call(
            "a2",
            "session.answer",
            json!({"session": session, "call": call, "decision": "once", "nested": 2}),
        )
        .await;
    home.until_turns(&session, 1).await;
    let listed = client.call("l", "package.list", json!({})).await;
    let removed: Vec<&str> = listed["result"]["packages"]
        .as_array()
        .expect("有")
        .iter()
        .filter(|item| item.get("removed").is_some())
        .filter_map(|item| item["package"].as_str())
        .collect();
    assert!(removed.is_empty(), "都装回来了：{removed:?}");
}

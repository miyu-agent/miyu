//! `command.catalog`（施工 O-6 补，`docs/construction/O-6-斜杠命令由核心解析（补）.md`）：不带会话列核心认的全部，照名字排，
//! 带别名、参数提示，照连接的语言，跟着包的照默认预设筛（施工 CC-1）；带会话的只列这个会话里打了不会被拒的（记忆关着的没有 `remember`）；场所会话不收，写错的
//! 编号是参数不对，没有的会话照找不到说。

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::memories::with_persona;
use crate::support::venues::bound_core;
use crate::support::*;

/// 列出来的名字，照先后。
fn names(reply: &Value) -> Vec<String> {
    reply["result"]["commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|command| command["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn without_a_session_every_command_is_listed_in_the_connections_language() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let reply = client.call("c1", "command.catalog", json!({})).await;
    assert_eq!(
        reply["result"]["commands"],
        json!([
            {"name": "clear", "aliases": ["reset"], "summary": "清空上下文"},
            {"name": "clear-memory", "aliases": ["reset-memory"], "summary": "清掉这个会话的记忆"},
            {"name": "dream", "aliases": [], "summary": "整理记忆", "argument": "[stop]", "usage": [
                {"form": "/dream", "summary": "现在就整理"},
                {"form": "/dream stop", "summary": "停止整理"},
            ]},
            {"name": "goal", "aliases": [], "summary": "长期目标", "argument": "[<目标> | pause | resume | clear]", "usage": [
                {"form": "/goal", "summary": "看进度"},
                {"form": "/goal <目标>", "summary": "设定长期目标；已经有一个就改它"},
                {"form": "/goal pause", "summary": "暂停"},
                {"form": "/goal resume", "summary": "继续"},
                {"form": "/goal clear", "summary": "清除"},
            ]},
            {"name": "loop", "aliases": [], "summary": "循环执行", "argument": "[<间隔>] <要说的话> | off", "usage": [
                {"form": "/loop <间隔> <要说的话>", "summary": "每隔一段时间说一次，例如 5m、2h"},
                {"form": "/loop <要说的话>", "summary": "由 AI 自己定下一次什么时候"},
                {"form": "/loop off", "summary": "停止此会话的循环"},
            ]},
            {"name": "remember", "aliases": [], "summary": "添加记忆", "argument": "<内容>"},
            {"name": "stop", "aliases": [], "summary": "全部停止"},
            {"name": "todo", "aliases": [], "summary": "待办", "argument": "[clear]", "usage": [
                {"form": "/todo", "summary": "看待办"},
                {"form": "/todo clear", "summary": "清空"},
            ]},
            {"name": "workspace", "aliases": [], "summary": "切换工作区", "argument": "<路径>"},
        ]),
        "{reply}"
    );
}

/// 不带会话的（施工 CC-1）：跟着包的命令照默认预设筛，默认预设没开那个包的不列（开会话的话敲了是 `unknown_command`）。
#[tokio::test]
async fn without_a_session_package_commands_follow_the_default_preset() {
    let home = Home::new();
    home.write("system/config.toml", "[preset]\ndefault = \"minimal\"\n");
    let core = crate::support::venues::configured_core(&home, &Script::new([]), Catalog::default());
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client.call("c1", "command.catalog", json!({})).await;
    let listed = names(&reply);
    for gone in ["goal", "loop", "todo"] {
        assert!(
            !listed.iter().any(|name| name == gone),
            "{gone}：{listed:?}"
        );
    }
    for kept in ["clear", "stop", "workspace"] {
        assert!(listed.iter().any(|name| name == kept), "{kept}：{listed:?}");
    }
}

#[tokio::test]
async fn with_a_session_only_what_would_be_accepted_is_listed() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    // 无人格的会话也有记忆（施工 P-12）：造会话时关掉才是没开。
    let session = client
        .create_with("c1", json!({"cwd": "~", "memory": "off"}))
        .await;
    let reply = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(
        names(&reply),
        ["clear", "stop", "todo", "workspace"],
        "这个会话没开记忆"
    );

    let home = Home::new();
    let mut client = Client::connect(with_persona(&home, &script, Catalog::default()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(
        names(&reply),
        [
            "clear",
            "clear-memory",
            "dream",
            "remember",
            "stop",
            "todo",
            "workspace"
        ]
    );
}

#[tokio::test]
async fn a_venue_session_and_a_bad_id_are_refused() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(bound_core(&home, &script, Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"].as_str().expect("有编号");
    let reply = client
        .call("c1", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("venue_session"), "{reply}");
    let reply = client
        .call("c2", "command.catalog", json!({"session": "nope"}))
        .await;
    assert_eq!(reply["error"]["code"], json!(-32602), "{reply}");
    let missing = "01900000-0000-7000-8000-00000000abcd";
    let reply = client
        .call("c3", "command.catalog", json!({"session": missing}))
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
}

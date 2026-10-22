//! 本机、远程分开（施工 Y-1 下，设计 `06-多用户与身份.md` 第八补第 5 条、U18）：网页软件替远程的人开的连接握手之前说
//! `connection.remote`，第二个本机入口的连接是本机的另一个人；这两种调本机才能用的方法一律回 `local_machine_only`，什么都不做，
//! 握手回应不给 `host`；本机的照常。只能往下标：握手以后说的不理。

use serde_json::{Value, json};

use miyu_endpoint::{Reach, serve_with};
use miyu_session::testkit::Script;

use crate::support::*;

/// 网页软件替远程的人开的连接：握手之前先说一句来自哪个地址，再握手。
async fn remote(home: &Home) -> (Client, Value) {
    let mut client = Client::connect(home.core(&Script::new([])));
    let notice =
        json!({"jsonrpc": "2.0", "method": "connection.remote", "params": {"from": "203.0.113.9"}});
    client.line(&notice.to_string()).await;
    let shook = client.hello().await;
    (client, shook)
}

/// 本机才能用的那几样，各带上让它成了本机才能用的那一格。
fn local_only_calls(home: &Home) -> Vec<(&'static str, Value)> {
    let dir = home.work.to_string_lossy().into_owned();
    let file = home.work.join("a.txt").to_string_lossy().into_owned();
    vec![
        ("fs.list", json!({"cwd": dir})),
        ("fs.find", json!({"root": dir, "query": "a", "wait": true})),
        ("fs.realpath", json!({"path": dir})),
        ("fs.read", json!({"path": file})),
        ("session.terminal", json!({"cwd": dir})),
        ("session.set_terminal", json!({"session": "x"})),
        ("mcp.login", json!({"name": "x"})),
        ("account.setup_code", json!({})),
        ("blob.put", json!({"path": file})),
        ("package.install", json!({"path": dir})),
        ("kb.mount", json!({"kb": "own", "folder": dir})),
        ("mcp.set", json!({"name": "x", "command": "sh"})),
        ("check", json!({"file": file})),
        (
            "config.trust",
            json!({"cwd": dir, "version": "x", "trust": true}),
        ),
        ("config.get", json!({"cwd": dir})),
        ("session.create", json!({"cwd": dir})),
        ("session.create", json!({"dirs": [dir]})),
        (
            "session.send",
            json!({"session": "x", "text": "hi", "cwd": dir}),
        ),
        (
            "session.send",
            json!({"session": "x", "text": "hi", "attachments": [{"path": file}]}),
        ),
        (
            "session.redo",
            json!({"session": "x", "attachments": [{"path": file}]}),
        ),
        ("session.set_workspace", json!({"session": "x", "cwd": dir})),
        (
            "command.run",
            json!({"session": "x", "text": "/help", "cwd": dir}),
        ),
        (
            "command.run",
            json!({"session": "x", "text": format!("/workspace {dir}")}),
        ),
        ("project.create", json!({"name": "p", "workspace": dir})),
        ("project.update", json!({"project": "p", "workspace": dir})),
    ]
}

/// 回应拒绝的原因。
fn reason_of(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

#[tokio::test]
async fn a_remote_connection_cannot_use_what_only_the_core_machine_has() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "secret").expect("写得进");
    let (mut client, shook) = remote(&home).await;
    assert!(shook.get("result").is_some(), "握手成了：{shook}");
    assert!(
        shook["result"].get("host").is_none(),
        "不给本机路径：{shook}"
    );
    for (n, (method, params)) in local_only_calls(&home).into_iter().enumerate() {
        let reply = client.call(&format!("r{n}"), method, params.clone()).await;
        assert_eq!(
            reason_of(&reply),
            Some("local_machine_only"),
            "{method} {params}：{reply}"
        );
    }
    // 不带本机路径的照常：`data` 的 `blob.put`、不写 `cwd` 的会话用它自己的默认工作区、只问在哪的 `/workspace`。
    let put = client
        .call("p1", "blob.put", json!({"data": "aGk=", "name": "hi.txt"}))
        .await;
    assert!(put.get("result").is_some(), "{put}");
    let created = client.call("c1", "session.create", json!({})).await;
    let session = created["result"]["session"].as_str().expect("造出来了");
    assert!(
        created["result"]["cwd"]
            .as_str()
            .is_some_and(|cwd| cwd.ends_with(session)),
        "会话用它自己的默认工作区：{created}"
    );
    let asked = client
        .call(
            "w1",
            "command.run",
            json!({"session": session, "text": "/workspace"}),
        )
        .await;
    assert_ne!(reason_of(&asked), Some("local_machine_only"), "{asked}");
}

/// 第二个本机入口来的（本机的另一个人）：同远程，本机才能用的拒、不给 `host`。
#[tokio::test]
async fn a_public_connection_is_not_the_core_machine_s_owner() {
    let home = Home::new();
    let (near, far) = tokio::io::duplex(64 * 1024);
    tokio::spawn(serve_with(far, home.core(&Script::new([])), Reach::Public));
    let mut client = Client::over(near);
    let shook = client.hello().await;
    assert!(shook["result"].get("host").is_none(), "{shook}");
    let dir = home.work.to_string_lossy().into_owned();
    let reply = client.call("r1", "fs.list", json!({"cwd": dir})).await;
    assert_eq!(reason_of(&reply), Some("local_machine_only"), "{reply}");
}

/// 本机的照常；握手以后才说 `connection.remote` 的不理，照旧是本机。
#[tokio::test]
async fn a_local_connection_keeps_its_host_and_cannot_be_marked_after_hello() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    let shook = client.hello().await;
    assert!(shook["result"].get("host").is_some(), "{shook}");
    let notice =
        json!({"jsonrpc": "2.0", "method": "connection.remote", "params": {"from": "203.0.113.9"}});
    client.line(&notice.to_string()).await;
    let dir = home.work.to_string_lossy().into_owned();
    let reply = client.call("r1", "fs.list", json!({"cwd": dir})).await;
    assert!(reply.get("result").is_some(), "本机的照常：{reply}");
}

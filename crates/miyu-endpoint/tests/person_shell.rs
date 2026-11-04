//! 人用 `!` 打的命令（施工 SH-1 上，设计 57，`docs/blueprint/protocol.md` 的 `session.send`）：真核心、真的基础系统。本人发
//! `shell: true` 的，记成写了 `shell` 的 `message.user`，不问权限、不进沙盒就跑，记 `shell.ran`，她接着回一轮、请求里是那条
//! 命令和输出；她自己的同一句照旧要问人。别的 harness、带附件的、空的、没有跑命令的工具的拒绝，会话里什么都不记。

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, ShellRan};
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};

use miyu_tool::Catalog;

use crate::support::venues::configured_core;
use crate::support::*;

/// 工具目录是出厂的基础系统的核心。
fn base(home: &Home, script: &Script) -> Client {
    let tools =
        Catalog::in_packages(miyu_basesystem::packages(&default_resources()).expect("出厂的工具"))
            .expect("合写法");
    Client::connect(configured_core(home, script, tools))
}

/// 人用 `!` 打 `text`。
async fn shell(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(
            id,
            "session.send",
            json!({"session": session, "text": text, "shell": true}),
        )
        .await
}

/// 日志里的 `shell.ran`。
fn rans(home: &Home, session: &str) -> Vec<ShellRan> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ShellRan(ran) => Some(ran),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_persons_command_runs_without_asking_and_she_reads_its_output() {
    let home = Home::new();
    let script = Script::new([Play::Says("看到了。")]);
    let mut client = base(&home, &script);
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = shell(&mut client, "c2", &session, "echo hi").await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    let message = log
        .iter()
        .find(|event| matches!(&event.body, Body::MessageUser(said) if said.shell))
        .map(|event| event.seq);
    let ran = rans(&home, &session);
    assert_eq!(ran.len(), 1, "{log:#?}");
    assert_eq!(Some(ran[0].message), message);
    assert_eq!((ran[0].exit, ran[0].reason.clone()), (Some(0), None));
    assert!(ran[0].text.contains("hi\n"), "{:?}", ran[0].text);
    assert!(
        !log.iter()
            .any(|event| matches!(event.body, Body::ApprovalRequested(_))),
        "人打的不问"
    );
    let requests = script.requests();
    let Some(Message::User { blocks }) = requests[0].1.messages.last() else {
        panic!("最后一条是人这边的");
    };
    let text: String = blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        text.contains("<user-shell reason=\"exited\">\n$ echo hi\nhi\nExit code 0.\n</user-shell>"),
        "{text}"
    );
}

#[tokio::test]
async fn only_the_person_sends_a_command_and_only_a_bare_one() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = base(&home, &script);
    client.hello().await;
    let session = client.create("c1", "~").await;
    let from = json!({"session": session, "text": "ls", "shell": true, "from": "claude-code"});
    let reply = client.call("s1", "session.send", from).await;
    assert_eq!(reason(&reply), Some("shell_not_allowed"), "{reply}");
    let blob = client
        .call("b1", "blob.put", json!({"name": "a.txt", "data": "aGk="}))
        .await;
    let attached = json!({"session": session, "text": "ls", "shell": true, "attachments": [blob["result"].clone()]});
    let reply = client.call("s2", "session.send", attached).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let reply = shell(&mut client, "s3", &session, "  ").await;
    assert_eq!(reason(&reply), Some("empty_message"), "{reply}");
    assert_eq!(home.log(&session).len(), 1, "什么都没记");
}

#[tokio::test]
async fn without_a_tool_that_runs_commands_it_is_unavailable() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = shell(&mut client, "s1", &session, "ls").await;
    assert_eq!(reason(&reply), Some("shell_unavailable"), "{reply}");
    assert_eq!(home.log(&session).len(), 1, "什么都没记");
}

#[tokio::test]
async fn interrupting_a_running_command_kills_it_and_she_does_not_reply() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = base(&home, &script);
    client.hello().await;
    let session = client.create("c1", "~").await;
    let sleep = if cfg!(windows) {
        "Write-Output first; Start-Sleep -Seconds 30"
    } else {
        "echo first; sleep 30"
    };
    let started = std::time::Instant::now();
    let reply = shell(&mut client, "s1", &session, sleep).await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    // 让它跑起来、印出第一行。
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let reply = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "send"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    let ran = rans(&home, &session);
    assert_eq!(ran.len(), 1);
    assert_eq!(
        ran[0].reason.as_ref().map(|reason| reason.as_str()),
        Some("interrupted")
    );
    assert!(ran[0].text.contains("first"), "{:?}", ran[0].text);
    assert!(started.elapsed().as_secs() < 25, "没等它睡完");
    assert!(script.requests().is_empty(), "她不回");
}

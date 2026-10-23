//! 会话认自己的属主（施工 O-4 上，`docs/construction/O-4-系统账号（上）.md`）：没在跑的会话照数据根里哪个账号的
//! `home/<账号>/sessions/<编号>/` 在找。施工 Y-2 上起只有属主碰得到（U2：管理员也看不到别人的会话）：把管理员造好的会话的
//! 目录挪到另一个账号下，换一个核心，管理员读页、订阅、说话、删掉一律当没有这个会话，那个账号的家目录一个字不动。系统账号
//! 照属主找、载入它自己家目录下的会话，见 `system_account.rs`。

use std::path::PathBuf;

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;

use crate::support::*;

/// 会话 `session` 在账号 `account` 名下的目录。
fn dir(home: &Home, account: &str, session: &str) -> PathBuf {
    home.root.session_dir(
        &AccountId::parse(account).expect("账号合写法"),
        &SessionId::parse(session).expect("会话编号合写法"),
    )
}

/// `dir` 里的日志结束了几轮。
fn turns_in(dir: &std::path::Path) -> usize {
    read_events(dir)
        .unwrap_or_default()
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .count()
}

#[tokio::test]
async fn a_session_under_another_account_is_found_loaded_and_deleted_there() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "a").await;
    home.until_turns(&session, 1).await;
    drop(client);
    first.stop_sessions().await;

    let (alices, bobs) = (dir(&home, "alice", &session), dir(&home, "bob", &session));
    std::fs::create_dir_all(bobs.parent().expect("有上一级")).expect("建得了");
    std::fs::rename(&alices, &bobs).expect("挪得动");
    // 策略快照这些 blob 也照属主放：系统账号的会话本来就在它自己的家目录下造，这里一起拷过去。
    let (alice, bob) = (
        AccountId::parse("alice").expect("合写法"),
        AccountId::parse("bob").expect("合写法"),
    );
    copy_tree(&home.root.blobs(&alice), &home.root.blobs(&bob));

    let second = home.core(&script);
    let mut client = Client::connect(second.clone());
    client.hello().await;
    let not_found =
        |reply: &serde_json::Value| reply["error"]["data"]["reason"] == json!("session_not_found");
    let page = client
        .call("p1", "view.page", json!({"session": session, "turns": 1}))
        .await;
    assert!(not_found(&page), "别人的会话读不了页：{page}");
    let mut watcher = Client::connect(second.clone());
    watcher.hello().await;
    let subscribed = watcher.subscribe("x1", &session).await;
    assert!(not_found(&subscribed), "订阅不了：{subscribed}");
    drop(watcher);
    let said = client.say("s2", &session, "b").await;
    assert!(not_found(&said), "说不了话：{said}");
    let deleted = client
        .call("d1", "session.delete", json!({"session": session}))
        .await;
    assert!(not_found(&deleted), "删不掉：{deleted}");
    // 每一个带会话编号的方法都一样（施工 Y-2 上：办之前那一道统一查，参数都不看）。`blob.get` 另有一套、拒的说法不同，
    // 见 `blob_session.rs`。
    let methods = [
        "command.catalog",
        "command.run",
        "events.append",
        "job.output",
        "job.stop",
        "session.answer",
        "session.clear",
        "session.compact",
        "session.configure",
        "session.context",
        "session.fork",
        "session.interrupt",
        "session.note",
        "session.recap",
        "session.redo",
        "session.respond",
        "session.revert",
        "session.set_meta",
        "session.set_permission_level",
        "session.set_terminal",
        "session.set_workspace",
        "session.terminal",
        "session.tools",
        "session.unrevert",
        "session.withdraw",
        "usage.query",
        "venue.records",
        "view.detail",
        "view.page",
    ];
    for (n, method) in methods.iter().enumerate() {
        let reply = client
            .call(&format!("m{n}"), method, json!({"session": session}))
            .await;
        assert!(not_found(&reply), "{method}：{reply}");
    }
    for (n, stream) in ["events", "view"].iter().enumerate() {
        let reply = client
            .call(
                &format!("w{n}"),
                "subscribe",
                json!({"stream": stream, "session": session}),
            )
            .await;
        assert!(not_found(&reply), "subscribe {stream}：{reply}");
    }
    assert_eq!(turns_in(&bobs), 1, "bob 的会话一个字没动");
    assert!(!alices.exists());
}

/// 把 `from` 整棵拷到 `to`。
fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("建得了");
    for entry in std::fs::read_dir(from).expect("读得了") {
        let entry = entry.expect("读得了");
        let target = to.join(entry.file_name());
        match entry.file_type().expect("有类型").is_dir() {
            true => copy_tree(&entry.path(), &target),
            false => {
                std::fs::copy(entry.path(), &target).expect("拷得了");
            }
        }
    }
}

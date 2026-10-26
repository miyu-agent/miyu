//! 成员照名单放（施工 Y-2 下下上，设计 06 U25「每一条路过同一道检查」）：红队那一面。成员碰不到机主的会话，整台机器的事和还没
//! 照人分开的一律 `not_permitted`，切不到完全放开，订不了配置；自己的会话照常用，说的话、发的命令记在他名下，用量只看得到自己的。

use serde_json::json;

use std::time::Duration;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;

use crate::support::login::*;
use crate::support::*;

#[tokio::test]
async fn a_member_reaches_only_his_own_things() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Says("嗯。"), Play::Says("好。")]));
    let mut owner = local(&core).await;
    let cwd = home.work.to_string_lossy().into_owned();
    let mine = owner.create("o1", &cwd).await;
    let mut bob = member(&core, "bob").await;

    // 机主的会话：照属主挡，说的是「没有这个会话」。
    for (method, params) in [
        ("session.send", json!({"session": mine, "text": "hi"})),
        ("view.page", json!({"session": mine})),
        ("session.interrupt", json!({"session": mine})),
        ("session.delete", json!({"session": mine})),
    ] {
        let reply = bob.call("x", method, params).await;
        assert_eq!(
            reason(&reply),
            Some("session_not_found"),
            "{method}: {reply}"
        );
    }
    // 整台机器的事、还没照人分开的：不给。
    for (method, params) in [
        ("package.install", json!({"package": "x"})),
        ("extension.enable", json!({"package": "x"})),
        ("provider.detect", json!({})),
        ("model.call", json!({})),
        ("check", json!({})),
        ("mcp.set", json!({})),
        ("account.invite", json!({})),
        ("subscribe", json!({"stream": "packages"})),
    ] {
        let reply = bob.call("x", method, params).await;
        assert_eq!(reason(&reply), Some("not_permitted"), "{method}: {reply}");
    }

    // 自己的会话照常：说的话记在他名下。
    let his = bob.call("c", "session.create", json!({})).await["result"]["session"]
        .as_str()
        .expect("造得出自己的会话")
        .to_string();
    bob.say("s", &his, "hi").await;
    let log = said(&home, &his).await;
    let said_by = log.iter().find_map(|event| match &event.body {
        Body::MessageUser(_) => Some(event.by.clone()),
        _ => None,
    });
    assert!(
        matches!(&said_by, Some(By::Person(person)) if person.account.as_str() == "bob"),
        "{said_by:?}"
    );
    // 完全放开只给管理员；工作区照常能切。
    let full = bob
        .call(
            "p",
            "session.set_permission_level",
            json!({"session": his, "level": "full"}),
        )
        .await;
    assert_eq!(reason(&full), Some("not_permitted"), "{full}");
    let work = bob
        .call(
            "q",
            "session.set_permission_level",
            json!({"session": his, "level": "workspace"}),
        )
        .await;
    assert_eq!(work["result"], json!({}), "{work}");
    // 用量按人记（施工 Y-2 下下中下）：机主也说一句；机主看得到两个人的、能照账号分开，成员只看得到他自己那一次。
    owner.say("o2", &mine, "hi").await;
    home.until_turns(&mine, 1).await;
    until_logged(&home, &his, |body| matches!(body, Body::TurnEnded(_))).await;
    let requests = |reply: &serde_json::Value| reply["result"]["rows"][0]["requests"].clone();
    let all_usage = owner.call("u", "usage.query", json!({})).await;
    assert_eq!(requests(&all_usage), json!(2), "{all_usage}");
    let split = owner
        .call("u", "usage.query", json!({"group": ["account"]}))
        .await;
    let rows: Vec<(serde_json::Value, serde_json::Value)> = split["result"]["rows"]
        .as_array()
        .expect("{split}")
        .iter()
        .map(|row| (row["account"].clone(), row["requests"].clone()))
        .collect();
    assert_eq!(
        rows,
        [(json!("alice"), json!(1)), (json!("bob"), json!(1))],
        "{split}"
    );
    let his_usage = bob.call("u", "usage.query", json!({})).await;
    assert_eq!(requests(&his_usage), json!(1), "{his_usage}");
    let his_split = bob
        .call("u", "usage.query", json!({"group": ["account"]}))
        .await;
    assert_eq!(
        his_split["result"]["rows"].as_array().map(Vec::len),
        Some(1),
        "成员照账号分也只有他自己：{his_split}"
    );
    // 订自己的视图照常。
    let view = bob
        .call("v", "subscribe", json!({"stream": "view", "session": his}))
        .await;
    assert!(view.get("result").is_some(), "{view}");
}

/// 等 bob 的会话 `session` 记下他说的那一句，交回日志。
async fn said(home: &Home, session: &str) -> Vec<Event> {
    until_logged(home, session, |body| matches!(body, Body::MessageUser(_))).await
}

/// 等 bob 的会话 `session` 的日志里有一条 `wanted` 的，交回日志。
async fn until_logged(home: &Home, session: &str, wanted: fn(&Body) -> bool) -> Vec<Event> {
    let bob = AccountId::parse("bob").expect("合写法");
    let id = SessionId::parse(session).expect("会话编号合写法");
    let dir = home.root.session_dir(&bob, &id);
    let read = || read_events(&dir).unwrap_or_default();
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while !read().iter().any(|event| wanted(&event.body)) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "六十秒内没等到");
    read()
}

/// 用量汇总是派生的（施工 8-15）：删掉重起照日志补回来，成员的会话也补（施工 Y-2 下下中下）。
#[tokio::test]
async fn a_rebuilt_ledger_still_counts_the_members_turns() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Says("嗯。")]));
    let mut bob = member(&core, "bob").await;
    let his = bob.call("c", "session.create", json!({})).await["result"]["session"]
        .as_str()
        .expect("造得出自己的会话")
        .to_string();
    bob.say("s", &his, "hi").await;
    until_logged(&home, &his, |body| matches!(body, Body::TurnEnded(_))).await;
    drop(bob);
    drop(core);
    let ledger = home.root.path().join("state").join(miyu_store::usage::FILE);
    assert!(ledger.exists(), "先记过：{}", ledger.display());
    // 第一份核心的后台还开着用量汇总，Windows 上删不掉开着的文件：把数据根拷一份、不带 `state/`（汇总、索引都是派生的），
    // 第二份核心在拷出来的那份上起。
    let fresh = Home::new();
    copy_without_state(home.root.path(), fresh.root.path());
    assert!(
        !fresh
            .root
            .path()
            .join("state")
            .join(miyu_store::usage::FILE)
            .exists()
    );
    let home = fresh;
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let split = owner
        .call("u", "usage.query", json!({"group": ["account"]}))
        .await;
    let rows: Vec<(serde_json::Value, serde_json::Value)> = split["result"]["rows"]
        .as_array()
        .expect("{split}")
        .iter()
        .map(|row| (row["account"].clone(), row["requests"].clone()))
        .collect();
    assert_eq!(rows, [(json!("bob"), json!(1))], "{split}");
}

/// 把数据根 `from` 拷进 `to`，`from` 下面的 `state/` 不拷。第一份核心的后台可能正在写临时文件（写完就改名）：列出来、拷的时候
/// 已经没了的跳过（Windows CI 上撞到过）。SQLite 的 `-shm` 不拷：它是共享内存的索引，Windows 上开着的库把它锁着一段（os error
/// 33），打开时照 `-wal` 重建。
fn copy_without_state(from: &std::path::Path, to: &std::path::Path) {
    fn gone(error: &std::io::Error) -> bool {
        error.kind() == std::io::ErrorKind::NotFound
    }
    fn copy(from: &std::path::Path, to: &std::path::Path, top: bool) {
        std::fs::create_dir_all(to).expect("建得出");
        let entries = match std::fs::read_dir(from) {
            Ok(entries) => entries,
            Err(error) if gone(&error) => return,
            Err(error) => panic!("读不出 {}：{error}", from.display()),
        };
        for entry in entries {
            let entry = entry.expect("读得出");
            if top && entry.file_name() == "state" {
                continue;
            }
            if entry.file_name().to_string_lossy().ends_with("-shm") {
                continue;
            }
            let target = to.join(entry.file_name());
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => copy(&entry.path(), &target, false),
                Ok(_) => match std::fs::copy(entry.path(), &target) {
                    Ok(_) => {}
                    Err(error) if gone(&error) => {}
                    Err(error) => panic!("拷不了 {}：{error}", entry.path().display()),
                },
                Err(error) if gone(&error) => {}
                Err(error) => panic!("看不出 {}：{error}", entry.path().display()),
            }
        }
    }
    copy(from, to, true);
}

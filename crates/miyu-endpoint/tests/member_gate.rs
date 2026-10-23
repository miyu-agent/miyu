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
        ("config.set", json!({"layer": "personal", "changes": []})),
        ("config.get", json!({"keys": ["models.chat"]})),
        ("package.install", json!({"package": "x"})),
        ("extension.enable", json!({"package": "x"})),
        ("secret.list", json!({})),
        ("provider.test", json!({"provider": "x"})),
        ("model.call", json!({})),
        ("check", json!({})),
        ("mcp.set", json!({})),
        ("account.invite", json!({})),
        ("subscribe", json!({"stream": "config"})),
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
    // 用量：机主也说一句；成员看不到机主的那一次（成员自己的会话这一步还不记账，Y-2 下下中按人记）。
    owner.say("o2", &mine, "hi").await;
    home.until_turns(&mine, 1).await;
    let requests = |reply: &serde_json::Value| reply["result"]["rows"][0]["requests"].clone();
    let all_usage = owner.call("u", "usage.query", json!({})).await;
    assert_eq!(requests(&all_usage), json!(1), "{all_usage}");
    let his_usage = bob.call("u", "usage.query", json!({})).await;
    assert_eq!(requests(&his_usage), json!(0), "{his_usage}");
    // 订自己的视图照常。
    let view = bob
        .call("v", "subscribe", json!({"stream": "view", "session": his}))
        .await;
    assert!(view.get("result").is_some(), "{view}");
}

/// 等 bob 的会话 `session` 记下他说的那一句，交回日志。
async fn said(home: &Home, session: &str) -> Vec<Event> {
    let bob = AccountId::parse("bob").expect("合写法");
    let id = SessionId::parse(session).expect("会话编号合写法");
    let dir = home.root.session_dir(&bob, &id);
    let read = || read_events(&dir).unwrap_or_default();
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while !read()
            .iter()
            .any(|event| matches!(event.body, Body::MessageUser(_)))
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "六十秒内没记下他说的话");
    read()
}

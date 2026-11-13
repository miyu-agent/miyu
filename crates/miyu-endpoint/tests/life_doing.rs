//! 别的会话知道她现在在干嘛（施工 LF-7，`docs/blueprint/life.md`「现在在干嘛」）：真核心、手拨的钟。她在人生会话里调
//! `schedule_wakeup` 写的 `reason`，同人格别的会话下一轮开头多一块，带这一句、写下的时刻、下次醒的时刻；不变的不重交，写了新的
//! 交新的，没写的清掉；`life.status` 带 `doing`。哪些会话交、停着的、压缩以后、中和、截短、重启以后在 `life_doing_more.rs`。

use std::sync::Arc;

use serde_json::json;

use miyu_endpoint::schedules::{Creator, Loop, Target, Wanted, When};
use miyu_kernel::block::Block;
use miyu_kernel::id::SessionId;
use miyu_kernel::request::Message;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};

use crate::support::clock::Hand;
use crate::support::doing::{START, core, doings, life_row, shell, wakeup};
use crate::support::*;

/// 十分钟（她每次定的间隔）。
const TEN: i64 = 600_000;

/// `life.start` 开出来的会话编号。
async fn start(client: &mut Client, persona: &str) -> String {
    let reply = client
        .call("l1", "life.start", json!({"persona": persona}))
        .await;
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没开出来：{reply}"))
        .to_string()
}

/// 一次请求里人这边的字，接成一串。
fn said(request: &miyu_kernel::request::Request) -> String {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.as_slice(),
            _ => &[],
        })
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn another_session_sees_it_once_and_again_when_she_writes_a_new_one() {
    let home = Home::new();
    let script = Script::new([
        wakeup(Some("在打零式\n两小时后回来")),
        Play::Says("好。"),
        Play::Says("在打零式。"),
        Play::Says("嗯。"),
        wakeup(Some("去散步")),
        Play::Says("走了。"),
        Play::Says("在散步。"),
        wakeup(None),
        Play::Says("又醒了。"),
    ]);
    let hand = Hand::at(START);
    let core = core(&home, &script, &hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let life = start(&mut client, "engineer").await;
    home.until_turns(&life, 1).await;
    let row = life_row(&core, &life);
    let doing = core.schedules().doing(&row.id).unwrap().expect("记下了");
    assert_eq!(doing.text, "在打零式 两小时后回来", "换行换成空格");
    assert_eq!(
        (doing.at.unix_millis(), doing.until.unix_millis()),
        (START, START + TEN)
    );
    let status = client
        .call("s1", "life.status", json!({"persona": "engineer"}))
        .await;
    assert_eq!(
        status["result"]["doing"],
        json!({"text": "在打零式 两小时后回来", "at": "2026-10-26T00:00:00.000Z"}),
        "{status}"
    );

    let cwd = home.work.to_string_lossy().into_owned();
    let other = client.create_as("c1", &cwd, "engineer").await;
    client.say("c2", &other, "在干嘛").await;
    home.until_turns(&other, 1).await;
    let first = shell(START, START + TEN, "在打零式 两小时后回来");
    assert_eq!(
        doings(&home, &other),
        [(first.clone(), vec![START.to_string()])]
    );
    let asked = script.requests();
    assert!(
        said(&asked.last().expect("请求过").1).contains(&first),
        "请求里有这一块"
    );

    client.say("c3", &other, "哦").await;
    home.until_turns(&other, 2).await;
    assert_eq!(doings(&home, &other).len(), 1, "不变的不重交");

    hand.advance(TEN);
    home.until_turns(&life, 2).await;
    client.say("c4", &other, "现在呢").await;
    home.until_turns(&other, 3).await;
    let second = shell(START + TEN, START + 2 * TEN, "去散步");
    assert_eq!(
        doings(&home, &other),
        [
            (first, vec![START.to_string()]),
            (second, vec![(START + TEN).to_string()])
        ],
        "写了新的交新的"
    );

    hand.advance(TEN);
    home.until_turns(&life, 3).await;
    assert_eq!(
        core.schedules().doing(&row.id).unwrap(),
        None,
        "她这一次没写 reason：清掉"
    );
    let status = client
        .call("s2", "life.status", json!({"persona": "engineer"}))
        .await;
    assert!(status["result"].get("doing").is_none(), "{status}");
    assert!(doings(&home, &life).is_empty(), "人生会话自己没有");
}

#[tokio::test]
async fn a_dynamic_loop_keeps_no_reason() {
    let home = Home::new();
    let script = Script::new([
        wakeup(Some("盯 CI")),
        Play::Says("等着。"),
        Play::Says("嗯。"),
    ]);
    let hand = Hand::at(START);
    let core = core(&home, &script, &hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let looping = client.create_as("c1", &cwd, "engineer").await;
    let at = Timestamp::from_unix_millis(START).unwrap();
    let row = core
        .schedules()
        .add(Wanted {
            by: Creator::Person,
            when: When::At { at },
            prompt: "看看 CI".to_string(),
            target: Target::Session {
                session: SessionId::parse(&looping).unwrap(),
            },
            looping: Some(Loop::Dynamic),
            first: Some(at),
        })
        .unwrap();
    home.until_turns(&looping, 1).await;
    let row = core.schedules().get(&row.id).unwrap();
    assert_eq!(
        row.next.map(|next| next.unix_millis()),
        Some(START + TEN),
        "她定的下一次照旧"
    );
    assert_eq!(
        core.schedules().doing(&row.id).unwrap(),
        None,
        "不记 reason"
    );
    let other = client.create_as("c3", &cwd, "engineer").await;
    client.say("c4", &other, "在吗").await;
    home.until_turns(&other, 1).await;
    assert!(doings(&home, &other).is_empty());
}

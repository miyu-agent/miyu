//! 整理的时候她正在写什么（施工 R-21，`docs/blueprint/memory.md` 第七条第 12 款）：整理记忆的模型是一台假服务器（`org/m`），
//! 订阅着这一间的（`Keeper::follow`）照她写完的先后收到改的、作废的、摘要写到第几个字，带着这一批的 `upto`；遮过 key；场所
//! 里听不见的那几条不推；不落盘；`merging` 带着这一批交的是哪几条。

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::id::{Seq, VenueId};
use miyu_kernel::time::UtcOffset;
use miyu_recall::merge::{Kind, Retire, Revised, Seen};
use miyu_recall::{MemoryEvent, MemoryId, from_event};
use miyu_session::{DreamFeed, Dreaming, Keeper, Listener, Turn};

use crate::support::extracting::*;
use crate::support::meaning::{persona, save};
use crate::support::*;

/// 整理记忆的那一家的 key（`organizer` 交的）：写进回答里的要遮掉。
const KEY: &str = "sk-org-0123456789abcdef";

fn id(n: u64) -> MemoryId {
    MemoryId::new(Seq::new(n).expect("从 1 起"))
}

fn keeper(home: &Home, listener: Listener) -> Keeper {
    Keeper::new(&home.memory, persona(), listener)
}

fn owner() -> Listener {
    Listener::Owner(alice())
}

/// 收到写完摘要的那一条为止（最多 10 秒），交回收到的。
async fn until_summary(feed: &mut DreamFeed) -> Vec<Dreaming> {
    let mut got = Vec::new();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(10), feed.next())
            .await
            .expect("十秒内收得到")
            .expect("核心那一头还在");
        let done = matches!(next.seen, Seen::Summary { done: true, .. });
        got.push(next);
        if done {
            return got;
        }
    }
}

async fn dream(home: &Home) {
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(home, owner())
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await
        .expect("整理了");
}

const SUMMARY: &str = "用户养了一只橘猫，叫团子，最近在学日语，准备明年考 N2。";

fn answer() -> String {
    serde_json::json!({
        "revised": [{"id": "m1", "text": format!("用户的 key 是 {KEY}")}],
        "retired": [{"id": "m2", "kind": "repeat", "by": "m3", "why": "和 m3 一样"}],
        "summary": SUMMARY
    })
    .to_string()
}

#[tokio::test]
async fn what_she_writes_comes_out_as_she_writes_it() {
    let server = Server::start(vec![stream(&answer())]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户的 key 放在备忘录里");
    save(&home, "用户养了一只猫");
    save(&home, "用户养了一只橘猫");
    let mut feed = keeper(&home, owner())
        .follow(Some(0), 1024)
        .expect("跟得上")
        .dreaming;
    dream(&home).await;
    let got = until_summary(&mut feed).await;
    let upto = Seq::new(3).expect("从 1 起");
    assert!(got.iter().all(|one| one.upto == upto), "{got:?}");
    let seen: Vec<Seen> = got.into_iter().map(|one| one.seen).collect();
    assert_eq!(
        seen,
        [
            Seen::Revised(Revised {
                id: id(1),
                text: "用户的 key 是 [REDACTED]".into()
            }),
            Seen::Retired(Retire {
                id: id(2),
                kind: Kind::Repeat,
                by: Some(id(3)),
                why: "和 m3 一样".into()
            }),
            Seen::Summary {
                chars: SUMMARY.chars().count(),
                done: true
            },
        ],
        "照写完的先后；改的遮了 key；摘要整段一下子来的，只交写完的那一次"
    );
    let logged = keeper(&home, owner())
        .follow(Some(0), 1024)
        .expect("跟得上");
    let kinds: Vec<&str> = logged
        .filled
        .iter()
        .map(|event| event.body.kind())
        .collect();
    assert!(
        !kinds.iter().any(|kind| kind.contains("dreaming")),
        "不落盘：{kinds:?}"
    );
    let merging = logged
        .filled
        .iter()
        .find_map(|event| match from_event(event)?.ok()? {
            MemoryEvent::Merging(merging) => Some(merging),
            _ => None,
        })
        .expect("记了开始合");
    assert_eq!(merging.given, [id(1), id(2), id(3)], "交的是哪几条");
}

#[tokio::test]
async fn a_place_sees_it_all_too() {
    let server = Server::start(vec![stream(&answer())]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户的 key 放在备忘录里");
    save(&home, "用户养了一只猫");
    save(&home, "用户养了一只橘猫");
    let place = Listener::Place(VenueId::parse("qq:group:1").expect("合写法"));
    let mut feed = keeper(&home, place)
        .follow(Some(0), 1024)
        .expect("跟得上")
        .dreaming;
    dream(&home).await;
    let seen: Vec<Seen> = until_summary(&mut feed)
        .await
        .into_iter()
        .map(|one| one.seen)
        .collect();
    assert!(
        matches!(seen.first(), Some(Seen::Revised(revised)) if revised.id.to_string() == "m1"),
        "主人那边记的场所里也推（施工 R-29：记忆是一个整体）：{seen:?}"
    );
    assert!(
        seen.iter().any(|one| matches!(one, Seen::Retired(retired) if retired.id.to_string() == "m2")),
        "{seen:?}"
    );
}

#[tokio::test]
async fn another_room_hears_nothing() {
    let server = Server::start(vec![stream(&answer())]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户的 key 放在备忘录里");
    save(&home, "用户养了一只猫");
    save(&home, "用户养了一只橘猫");
    let other = miyu_store::recall::Room::persona(&alice_account(), "writer");
    let mut elsewhere = Keeper::new(&home.memory, other, owner())
        .follow(Some(0), 1024)
        .expect("跟得上")
        .dreaming;
    let mut here = keeper(&home, owner())
        .follow(Some(0), 1024)
        .expect("跟得上")
        .dreaming;
    dream(&home).await;
    until_summary(&mut here).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(200), elsewhere.next())
            .await
            .is_err(),
        "别的人格那一间的不推"
    );
}

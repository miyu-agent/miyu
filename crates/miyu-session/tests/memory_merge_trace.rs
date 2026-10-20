//! 整理留痕（施工 R-17，`docs/blueprint/memory.md` 第七条第 10 款）：真记忆日志；整理记忆的模型是一台假服务器（`org/m`）。
//!
//! 每一批发请求以前记 `ext.memory.merging`（合到哪、这一批几条新记的、还有几条、是不是人叫的），合完的 `merged`、没合成的
//! `merge_failed` 带同一个 `upto`；作废的写哪一种、留下的是哪一条；放不下的分两批，`left` 往下数；连着第三次没成的另记跳过；
//! 核心半路停了的，下一回第一次开这一间补一条 `interrupted`。

use std::sync::Arc;

use miyu_http::testkit::Server;
use miyu_kernel::id::Seq;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{
    INTERRUPTED, MemoryEvent, MemoryId, MergeFailed, Merged, Merging, RetireKind, Retired,
    from_event,
};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Dreamed, Keeper, Listener, NotDreamed, Turn};
use miyu_store::memory::MemoryLogs;

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona, save};
use crate::support::*;

fn seq(n: u64) -> Seq {
    Seq::new(n).expect("从 1 起")
}

fn id(n: u64) -> MemoryId {
    MemoryId::new(seq(n))
}

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

/// 现在就整理这一间：照场地这时的配置、属主 alice、东九区。
async fn dream(home: &Home) -> Result<Dreamed, NotDreamed> {
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(home)
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await
}

/// 这一间记忆日志里整理那几种（`merging`、`merged`、`merge_failed`、`retired`），照先后。
fn trace(home: &Home) -> Vec<MemoryEvent> {
    let following = keeper(home).follow(Some(0), 1024).expect("跟得上");
    following
        .filled
        .iter()
        .filter_map(|event| from_event(event)?.ok())
        .filter(|event| {
            matches!(
                event,
                MemoryEvent::Merging(_)
                    | MemoryEvent::Merged(_)
                    | MemoryEvent::MergeFailed(_)
                    | MemoryEvent::Retired(_)
            )
        })
        .collect()
}

/// 开始合一批：交的是 `given` 这几条（施工 R-21；手记的、R-21 以前的没有）。
fn merging(upto: u64, fresh: u32, left: u32, manual: bool, given: &[u64]) -> MemoryEvent {
    MemoryEvent::Merging(Merging {
        upto: seq(upto),
        fresh,
        left,
        manual,
        given: given.iter().map(|n| id(*n)).collect(),
        session: None,
    })
}

#[tokio::test]
async fn a_dream_writes_where_it_starts_and_what_it_retired() {
    let server = Server::start(vec![stream(
        &serde_json::json!({
            "retired": [
                {"id": "m1", "kind": "repeat", "by": "m2", "why": "和 m2 一样"},
                {"id": "m3", "kind": "worthless", "why": "一次性的"}
            ],
            "summary": "用户养了一只橘猫。"
        })
        .to_string(),
    )])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户养了一只猫");
    save(&home, "用户养了一只橘猫");
    save(&home, "用户让她今天提醒一下开会");
    dream(&home).await.expect("整理了");
    assert_eq!(
        trace(&home),
        [
            merging(3, 3, 3, true, &[1, 2, 3]),
            MemoryEvent::Retired(Retired {
                id: id(1),
                why: "和 m2 一样".into(),
                kind: Some(RetireKind::Repeat),
                kept: Some(id(2)),
            }),
            MemoryEvent::Retired(Retired {
                id: id(3),
                why: "一次性的".into(),
                kind: Some(RetireKind::Worthless),
                kept: None,
            }),
            MemoryEvent::Merged(Merged {
                upto: seq(3),
                given: 3,
                revised: 0,
                retired: 2,
                failed: false,
            }),
        ],
        "发以前先记开始合，作废的写哪一种、留下哪一条，合完的带同一个 upto"
    );
    assert_eq!(book_merging(&home), None, "收尾了");
}

#[tokio::test]
async fn a_merge_in_the_background_is_not_manual() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        stream(&serde_json::json!({"summary": "用户养猫。"}).to_string()),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 1\n");
    let handle = home
        .create_full(
            &Script::new([Play::Says("好。"), Play::Says("好。")]),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "橘猫").await;
    requested(&server, 2).await;
    for _ in 0..200 {
        if trace(&home)
            .iter()
            .any(|event| matches!(event, MemoryEvent::Merged(_)))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let started: Vec<Merging> = trace(&home)
        .into_iter()
        .filter_map(|event| match event {
            MemoryEvent::Merging(merging) => Some(merging),
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 1, "{started:?}");
    assert!(!started[0].manual, "后台自己合的");
    assert_eq!((started[0].fresh, started[0].left), (2, 2));
    stop(&handle).await;
}

#[tokio::test]
async fn every_failure_is_written_and_the_third_skips_the_batch() {
    let server = Server::start(vec![
        stream("not json"),
        stream("still not json"),
        stream("nope"),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户养了一只猫");
    for _ in 0..3 {
        assert!(matches!(dream(&home).await, Err(NotDreamed::Failed(_))));
    }
    let events = trace(&home);
    let failed: Vec<&MergeFailed> = events
        .iter()
        .filter_map(|event| match event {
            MemoryEvent::MergeFailed(failed) => Some(failed),
            _ => None,
        })
        .collect();
    assert_eq!(
        failed.iter().map(|one| one.tries).collect::<Vec<_>>(),
        [Some(1), Some(2), Some(3)],
        "每一次没成都记，连着第几次"
    );
    assert!(
        failed
            .iter()
            .all(|one| one.upto == seq(1) && !one.why.is_empty())
    );
    assert_eq!(
        events.last(),
        Some(&MemoryEvent::Merged(Merged {
            upto: seq(1),
            given: 1,
            revised: 0,
            retired: 0,
            failed: true,
        })),
        "第三次另记跳过这一批"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, MemoryEvent::Merging(_)))
            .count(),
        3,
        "每一次都先记开始合"
    );
    assert_eq!(book_merging(&home), None);
}

#[tokio::test]
async fn what_does_not_fit_goes_next_and_left_counts_down() {
    let server = Server::start(vec![
        stream("{}"),
        stream("{}"),
        // 多备一个：要是又发了，看得出来。
        stream("{}"),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    for n in 0..100 {
        save(&home, &format!("第 {n} 条：{}", "很长的一句话".repeat(19)));
    }
    dream(&home).await.expect("整理了");
    let started: Vec<Merging> = trace(&home)
        .into_iter()
        .filter_map(|event| match event {
            MemoryEvent::Merging(merging) => Some(merging),
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 2, "放不下的分两批：{started:?}");
    assert_eq!(started[0].left, 100, "头一批的 left 是总数");
    assert_eq!(
        started[1].left,
        100 - started[0].fresh,
        "第二批的 left 是剩下的"
    );
    assert_eq!(started[0].fresh + started[1].fresh, 100, "各批加起来是全部");
    assert!(started.iter().all(|one| one.manual));
    assert_eq!(started[1].upto, seq(100));
}

#[tokio::test]
async fn an_unfinished_batch_is_closed_when_the_room_is_next_opened() {
    let home = Home::new();
    // 上一回的核心：开始合了一批、没收尾就停了。另一份登记就是另一回。
    {
        let earlier = MemoryLogs::new(&home.root);
        let (log, _) = earlier.open(&persona()).expect("开得了");
        drop(log);
        save_in(&earlier, "用户养了一只猫");
        let (log, _) = earlier.open(&persona()).expect("开得了");
        log.append(now(), alice(), None, &merging(1, 1, 1, false, &[]))
            .expect("记得下");
    }
    let events = trace(&home);
    assert_eq!(
        events.last(),
        Some(&MemoryEvent::MergeFailed(MergeFailed {
            upto: seq(1),
            why: INTERRUPTED.into(),
            tries: None,
        })),
        "这一回第一次开：补一条半路停了的"
    );
    assert_eq!(book_merging(&home), None);
    assert_eq!(trace(&home), events, "只补一次");
}

#[tokio::test]
async fn a_finished_batch_is_left_alone_when_the_room_is_next_opened() {
    let home = Home::new();
    {
        let earlier = MemoryLogs::new(&home.root);
        save_in(&earlier, "用户养了一只猫");
        let (log, _) = earlier.open(&persona()).expect("开得了");
        for event in [
            merging(1, 1, 1, true, &[]),
            MemoryEvent::Merged(Merged {
                upto: seq(1),
                given: 1,
                revised: 0,
                retired: 0,
                failed: false,
            }),
        ] {
            log.append(now(), alice(), None, &event).expect("记得下");
        }
    }
    assert!(
        !trace(&home)
            .iter()
            .any(|event| matches!(event, MemoryEvent::MergeFailed(_))),
        "收了尾的不补"
    );
}

/// 底账里开始合了、还没收尾的那一批。
fn book_merging(home: &Home) -> Option<Seq> {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.book(miyu_recall::MemoryBook::merging)
}

/// 在另一份登记 `logs` 里记一条人记的。
fn save_in(logs: &MemoryLogs, text: &str) {
    let (log, _) = logs.open(&persona()).expect("开得了");
    let saved = miyu_recall::Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![alice()],
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
    };
    log.append(now(), alice(), None, &MemoryEvent::Saved(saved))
        .expect("记得下");
}

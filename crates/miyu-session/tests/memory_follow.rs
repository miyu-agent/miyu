//! 跟着看一间的记忆日志（施工 R-12 上，`docs/blueprint/memory.md`「协议」的流 `memory`）：照谁在听挑（施工 R-13 上：场所里只冒出
//! 在这个场所说过的），冒不出来的那一条和作废它的不给，整间的（抽到哪、合并、摘要、清空）照给；补的和之后的一个挑法；清掉的哪里都不出来：补的时候清掉的那几条、清空以前的
//! 摘要不给，出处全死了的也不给；攒满了算掉队、不再交。

use std::sync::atomic::Ordering;

use miyu_kernel::id::{Seq, SessionId, TurnId, VenueId};
use miyu_kernel::origin::By;
use miyu_recall::{
    Cleared, MemoryEvent, MemoryId, Merged, Restored, Retired, Saved, Source, Summary, Tagged,
};
use miyu_session::{Keeper, Listener};

use crate::support::meaning::{persona, save};
use crate::support::*;

fn append(home: &Home, by: By, event: MemoryEvent) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.append(now(), by, None, &event).expect("记得下");
}

fn place(n: u32) -> VenueId {
    VenueId::parse(&format!("qq:group:{n}")).expect("合写法")
}

/// 在群 `n` 里说的一条（施工 R-13 上：照场所冒出来）。
fn said_in(home: &Home, n: u32, text: &str) {
    append(
        home,
        alice(),
        MemoryEvent::Saved(Saved {
            class: "user".into(),
            text: text.into(),
            sources: Vec::new(),
            audience: Vec::new(),
            replaces: None,
            about: None,
            venue: Some(place(n)),
            keywords: None,
        }),
    );
}

fn seqs(events: &[miyu_kernel::event::Event]) -> Vec<u64> {
    events.iter().map(|event| event.seq.get()).collect()
}

#[tokio::test]
async fn every_line_follows_anywhere() {
    let home = Home::new();
    said_in(&home, 1, "小林养了一只猫");
    said_in(&home, 2, "只有群里那个人听得到的");
    append(
        &home,
        alice(),
        MemoryEvent::Retired(Retired {
            id: MemoryId::parse("m2").expect("合写法"),
            why: "不要了".into(),
            kind: None,
            kept: None,
        }),
    );
    let merged = || {
        MemoryEvent::Merged(Merged {
            upto: miyu_kernel::id::Seq::new(1).expect("从 1 起"),
            given: 1,
            revised: 0,
            retired: 0,
            failed: false,
        })
    };
    append(&home, alice(), merged());
    let keeper = Keeper::new(&home.memory, persona(), Listener::Place(place(1)));
    let mut following = keeper.follow(Some(0), 8).expect("跟得上");
    assert_eq!(
        seqs(&following.filled),
        [1, 2, 3, 4],
        "别的群里记的、作废它的、整间的都补（施工 R-29：记忆是一个整体）"
    );
    assert_eq!(following.upto, 4);

    said_in(&home, 1, "小林住在杭州");
    said_in(&home, 2, "又一条听不到的");
    append(&home, alice(), merged());
    let mut live = Vec::new();
    while let Ok(event) = following.live.try_recv() {
        live.push(event);
    }
    assert_eq!(seqs(&live), [5, 6, 7], "之后的一个挑法");
    assert!(!following.lagged.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_full_queue_is_a_lag_and_stops() {
    let home = Home::new();
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    let mut following = keeper.follow(None, 1).expect("跟得上");
    for text in ["一", "二", "三"] {
        save(&home, text);
    }
    assert_eq!(
        following.live.recv().await.map(|event| event.seq.get()),
        Some(1)
    );
    assert_eq!(following.live.recv().await, None, "放不进的那一刻起不再交");
    assert!(following.lagged.load(Ordering::SeqCst), "是掉了队");
}

fn summary(text: &str, upto: u64) -> MemoryEvent {
    MemoryEvent::Summary(Summary {
        text: text.into(),
        upto: Seq::new(upto).expect("从 1 起"),
    })
}

fn cleared(session: Option<SessionId>) -> MemoryEvent {
    MemoryEvent::Cleared(Cleared { session })
}

#[tokio::test]
async fn what_was_cleared_is_not_filled_in() {
    let home = Home::new();
    save(&home, "用户养了一只猫");
    append(&home, alice(), summary("用户养猫", 1));
    append(&home, alice(), cleared(None));
    save(&home, "用户住在杭州");
    append(&home, alice(), summary("用户住在杭州", 4));
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    for after in [0, 1] {
        let following = keeper.follow(Some(after), 8).expect("跟得上");
        assert_eq!(
            seqs(&following.filled),
            [3, 4, 5],
            "清掉的那一条、清空以前的摘要不补；清空、之后的照补（after {after}）"
        );
    }
    let mut following = keeper.follow(None, 8).expect("跟得上");
    append(&home, alice(), cleared(None));
    let live = following.live.try_recv().expect("推了");
    assert_eq!(live.seq.get(), 6, "之后的清空照推，头照它重新列");
}

#[tokio::test]
async fn a_cleared_session_hides_only_what_came_from_it() {
    let home = Home::new();
    let parse = |text: &str| SessionId::parse(text).expect("合写法");
    let (cleared_one, other) = (
        parse("0192f3a0-1111-7abc-8def-001122334455"),
        parse("0192f3a0-3333-7abc-8def-001122334455"),
    );
    // 两个会话都在：出处活着，挑掉的只因为清了。
    for session in [&cleared_one, &other] {
        std::fs::create_dir_all(home.root.session_dir(&alice_account(), session)).expect("建得了");
    }
    let from = |session: &SessionId| {
        MemoryEvent::Saved(Saved {
            class: "user".into(),
            text: "从那个会话来的".into(),
            sources: vec![Source {
                session: session.clone(),
                turn: TurnId::new(Seq::FIRST),
            }],
            audience: vec![alice()],
            replaces: None,
            about: None,
            venue: None,
            keywords: None,
        })
    };
    append(&home, alice(), from(&cleared_one));
    append(&home, alice(), from(&other));
    save(&home, "人记的");
    append(
        &home,
        alice(),
        MemoryEvent::Retired(Retired {
            id: MemoryId::parse("m1").expect("合写法"),
            why: "不对".into(),
            kind: None,
            kept: None,
        }),
    );
    append(&home, alice(), cleared(Some(cleared_one)));
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    let following = keeper.follow(Some(0), 8).expect("跟得上");
    assert_eq!(
        seqs(&following.filled),
        [2, 3, 5],
        "清掉的那一条和作废它的不补；别的会话来的、人记的、清空照补"
    );
}

#[tokio::test]
async fn a_memory_whose_sources_are_gone_is_not_filled_in() {
    let home = Home::new();
    // 这个会话的目录不在（删了、清出回收处）：出处死了。
    let gone = SessionId::parse("0192f3a0-2222-7abc-8def-001122334455").expect("合写法");
    append(
        &home,
        alice(),
        MemoryEvent::Saved(Saved {
            class: "user".into(),
            text: "从删掉的会话来的".into(),
            sources: vec![Source {
                session: gone,
                turn: TurnId::new(Seq::FIRST),
            }],
            audience: vec![alice()],
            replaces: None,
            about: None,
            venue: None,
            keywords: None,
        }),
    );
    save(&home, "人记的");
    append(
        &home,
        alice(),
        MemoryEvent::Retired(Retired {
            id: MemoryId::parse("m1").expect("合写法"),
            why: "不对".into(),
            kind: None,
            kept: None,
        }),
    );
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    let following = keeper.follow(Some(0), 8).expect("跟得上");
    assert_eq!(
        seqs(&following.filled),
        [2],
        "出处全死了的那一条和作废它的当它不在"
    );
}

/// 恢复的说的是一条（施工 R-12 下）：挑法同作废，别的群里记的那一条的恢复照给（施工 R-29）。
#[tokio::test]
async fn a_restoration_is_heard_like_a_retirement() {
    let home = Home::new();
    said_in(&home, 1, "小林养了一只猫");
    said_in(&home, 2, "只有群里那个人听得到的");
    for n in [1, 2] {
        let id = MemoryId::parse(&format!("m{n}")).expect("合写法");
        append(
            &home,
            alice(),
            MemoryEvent::Retired(Retired {
                id,
                why: "错了".into(),
                kind: None,
                kept: None,
            }),
        );
        append(&home, alice(), MemoryEvent::Restored(Restored { id }));
    }
    let keeper = Keeper::new(&home.memory, persona(), Listener::Place(place(1)));
    let following = keeper.follow(Some(0), 8).expect("跟得上");
    assert_eq!(
        seqs(&following.filled),
        [1, 2, 3, 4, 5, 6],
        "别的群里记的那一条的作废、恢复也补"
    );
}

/// 换关键词的说的是一条（施工 R-15 上）：挑法同作废，别的群里记的那一条换的关键词照给（施工 R-29）。
#[tokio::test]
async fn a_tagging_is_heard_like_a_retirement() {
    let home = Home::new();
    said_in(&home, 1, "小林养了一只猫");
    said_in(&home, 2, "只有群里那个人听得到的");
    for n in [1, 2] {
        let id = MemoryId::parse(&format!("m{n}")).expect("合写法");
        append(
            &home,
            alice(),
            MemoryEvent::Tagged(Tagged {
                id,
                keywords: "宠物".into(),
            }),
        );
    }
    let keeper = Keeper::new(&home.memory, persona(), Listener::Place(place(1)));
    let following = keeper.follow(Some(0), 8).expect("跟得上");
    assert_eq!(
        seqs(&following.filled),
        [1, 2, 3, 4],
        "别的群里记的那一条换的关键词也补"
    );
}

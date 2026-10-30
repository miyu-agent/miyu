//! 认识的人那一类（施工 E-1 上，设计 42）在 `remember`、`memory_search` 里的样子：要写是谁、别的类不收、搜到的一行接是谁；
//! 搜到的人卡一张一行（施工 E-3）。端口是假的，同 `tools.rs`。

use std::sync::{Arc, PoisonError};

use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_tool::{FoundCard, FoundMemory, FoundTurn, Searched};

use crate::past::call_reading;
use crate::tools::{Fake, at, call, id, text};

/// 认识的人（施工 E-1 上）：要写是谁，没写的不记、说一句；写了的照收（去掉前后空白）；别的类写了是谁也不收；改一条没写是谁的
/// 照收（端口带上旧的）。
#[tokio::test]
async fn a_person_says_who() {
    let port = Arc::new(Fake::default());
    let done = call(
        Some(Arc::clone(&port)),
        "remember",
        serde_json::json!({"class":"person","text":"小林用 RX 7800 XT"}),
    )
    .await;
    assert!(done.error, "没写是谁");
    assert_eq!(
        text(&done),
        "A person memory needs who: their name, or an id like qq:10086 when known."
    );
    for (args, expect) in [
        (
            serde_json::json!({"class":"person","text":"小林用 RX 7800 XT","who":" qq:2 "}),
            Some("qq:2"),
        ),
        (
            serde_json::json!({"class":"user","text":"用户用 N 卡","who":"小林"}),
            None,
        ),
        (
            serde_json::json!({"class":"person","text":"小林换了 9070","replaces":"m2"}),
            None,
        ),
    ] {
        let done = call(Some(Arc::clone(&port)), "remember", args).await;
        assert!(!done.error, "{}", text(&done));
        let saved = port
            .saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last()
            .map(|remember| remember.who.clone());
        assert_eq!(saved, Some(expect.map(str::to_string)));
    }
    assert_eq!(
        port.saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len(),
        3
    );
}

/// 搜到的认识的人那一行类后面接是谁（施工 E-1 上）。
#[tokio::test]
async fn a_person_found_shows_who() {
    let port = Arc::new(Fake {
        found: Searched {
            memories: vec![FoundMemory {
                id: id(4),
                class: "person".into(),
                text: "小林用 RX 7800 XT".into(),
                at: at(5),
                retired: false,
                who: Some("qq:2".into()),
                venue: None,
                place: None,
            }],
            turns: Vec::new(),
            cards: Vec::new(),
        },
        ..Fake::default()
    });
    let done = call(
        Some(port),
        "memory_search",
        serde_json::json!({"query":"显卡"}),
    )
    .await;
    assert!(
        text(&done).starts_with("m4 person qq:2 "),
        "{}",
        text(&done)
    );
}

fn card(who: &str, names: &[&str], text: &str) -> FoundCard {
    FoundCard {
        who: who.into(),
        names: names.iter().map(ToString::to_string).collect(),
        text: text.into(),
        at: at(20),
    }
}

/// 搜到的人卡（施工 E-3）：一张一行（`card of 是谁 (叫法): 正文`，换行换成空格），排在记下的后面、以前的对话前面（用到几次
/// 照开头那几行的编号数）；给人看的那一句多卡的张数；给头的那一份多 `cards`。
#[tokio::test]
async fn cards_found_come_after_memories() {
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法");
    let port = Arc::new(Fake {
        found: Searched {
            memories: vec![FoundMemory {
                id: id(4),
                class: "person".into(),
                text: "小林用 RX 7800 XT".into(),
                at: at(5),
                retired: false,
                who: Some("qq:2".into()),
                venue: None,
                place: None,
            }],
            turns: vec![FoundTurn {
                session,
                turn: TurnId::new(Seq::new(8).expect("从 1 起")),
                at: at(6),
                text: "小林来了".into(),
                venue: None,
                opens: true,
            }],
            cards: vec![
                card(
                    "qq:2",
                    &["小林", "林林"],
                    "群里认识的摄影爱好者。\n下周搬去大阪。",
                ),
                card("qq:3", &[], "写 Rust 的群友。"),
            ],
        },
        ..Fake::default()
    });
    let done = call_reading(
        port,
        "memory_search",
        serde_json::json!({"query":"你认识小林吗"}),
    )
    .await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "m4 person qq:2 2026-10-05: 小林用 RX 7800 XT\n\
         card of qq:2 (小林, 林林): 群里认识的摄影爱好者。 下周搬去大阪。\n\
         card of qq:3: 写 Rust 的群友。\n\
         2026-10-06 session 22334455: 小林来了"
    );
    let human = done.human.as_ref().expect("有给人看的");
    assert_eq!(human.key, "software/memory/memory_search/found-cards");
    assert_eq!(
        serde_json::to_value(human).expect("写得出")["fields"],
        serde_json::json!({"cards": "2", "memories": "1", "turns": "1"})
    );
    assert_eq!(
        done.detail.as_ref().expect("有")["cards"],
        serde_json::json!([
            {"who": "qq:2", "names": ["小林", "林林"], "date": "2026-10-20", "text": "群里认识的摄影爱好者。\n下周搬去大阪。"},
            {"who": "qq:3", "names": [], "date": "2026-10-20", "text": "写 Rust 的群友。"},
        ])
    );
}

/// 只搜到卡的不说没找到（施工 E-3）。
#[tokio::test]
async fn only_a_card_is_not_nothing() {
    let port = Arc::new(Fake {
        found: Searched {
            cards: vec![card("qq:2", &["小林"], "摄影爱好者。")],
            ..Searched::default()
        },
        ..Fake::default()
    });
    let done = call(
        Some(port),
        "memory_search",
        serde_json::json!({"query":"小林"}),
    )
    .await;
    assert_eq!(text(&done), "card of qq:2 (小林): 摄影爱好者。");
}

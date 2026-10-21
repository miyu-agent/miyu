//! 认识的人那一类（施工 E-1 上，设计 42）在 `remember`、`memory_search` 里的样子：要写是谁、别的类不收、搜到的一行接是谁。
//! 端口是假的，同 `tools.rs`。

use std::sync::{Arc, PoisonError};

use miyu_tool::{FoundMemory, Searched};

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
        "Say who this person memory is about in who: their name, or their id like qq:10086 when you see one."
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
            }],
            turns: Vec::new(),
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

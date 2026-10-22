//! 人卡（施工 E-1 下，`docs/blueprint/memory.md` 第七条第 15 款，设计 42）：整理时这一批里有认识的人的，请求最后接上合卡的那一段
//! 和他们现在的卡；交回的卡只收这一批里的人，记成 `ext.memory.card`，叫法接上认出来的名字和旧卡上的；和现在那一张一样的不再记。
//! 她当场记的名字照出处那一轮的会话日志认出平台编号。没有认识的人的批不多那一段。整理记忆的模型是一台假服务器。

use std::sync::Arc;

use miyu_http::testkit::Server;
use miyu_kernel::event::Event;
use miyu_kernel::id::{SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{Card, MemoryEvent, Saved, Source, from_event};
use miyu_session::{Dreamed, Keeper, Listener, NotDreamed, Turn};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};

use crate::support::extracting::*;
use crate::support::meaning::{persona, save};
use crate::support::*;

/// 合卡的那一段的开头（`resources/software/memory/merge/people.txt`）。
const PEOPLE: &str = "People you know in these memories:";

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

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

/// 记一条认识的人：`by` 写它的那一方，出处是 `source` 的话。
fn person(home: &Home, by: By, who: &str, text: &str, source: Option<Source>) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "person".into(),
        text: text.into(),
        sources: source.into_iter().collect(),
        audience: vec![alice()],
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: Some(who.into()),
    };
    log.append(now(), by, None, &MemoryEvent::Saved(saved))
        .expect("记得下");
}

/// 群里的一个会话：qq:2 以群名片「小林」说了一句（第 2 轮）。交回那一轮。
fn group_source(home: &Home) -> Source {
    let session = SessionId::parse("0192f3a0-2222-7abc-8def-001122334455").expect("合写法");
    let dir = home.root.session_dir(&alice_account(), &session);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了日志");
    let said = Event::from_line(
        r#"{"seq":1,"at":"2026-10-22T03:00:00.000Z","kind":"message.user","by":{"kind":"external","venue":"qq:group:5","id":"qq:2"},"body":{"blocks":[{"type":"text","text":"我喜欢拍照"}],"venue":{"msg":"9","name":"小林"}}}"#,
    )
    .expect("合写法");
    log.append(&[said]).expect("写得进");
    Source {
        session,
        turn: TurnId::new(miyu_kernel::id::Seq::new(2).expect("从 1 起")),
    }
}

fn tool() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

/// 这一间记忆日志里的人卡事件，照先后。
fn cards(home: &Home) -> Vec<Card> {
    let following = keeper(home).follow(Some(0), 4096).expect("跟得上");
    following
        .filled
        .iter()
        .filter_map(|event| match from_event(event)?.ok()? {
            MemoryEvent::Card(card) => Some(card),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_dream_writes_a_card_for_the_people_in_the_batch() {
    let first = r#"{"cards": [
        {"who": "qq:2", "text": "小林是群友，喜欢拍照。", "names": ["林林"]},
        {"who": "qq:999", "text": "这一批里没有他"}
    ]}"#;
    let same =
        r#"{"cards": [{"who": "qq:2", "text": "小林是群友，喜欢拍照。", "names": ["林林"]}]}"#;
    let server = Server::start(vec![stream(first), stream(same)]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let source = group_source(&home);
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let old = Card {
        who: "qq:2".into(),
        text: "在群里认识的。".into(),
        names: vec!["阿林".into()],
    };
    log.append(now(), alice(), None, &MemoryEvent::Card(old))
        .expect("记得下");
    person(&home, tool(), "小林", "小林喜欢拍照", Some(source));

    dream(&home).await.expect("整理了");
    let asked = asked(&server, 0);
    assert!(
        asked.contains(" person qq:2 "),
        "她记的名字照会话日志认成平台编号：{asked}"
    );
    assert!(asked.contains(&format!("{PEOPLE} qq:2.")), "{asked}");
    assert!(
        asked.ends_with("Their current cards:\nqq:2: 在群里认识的。"),
        "接着他现在的卡写：{asked}"
    );
    let written = cards(&home);
    assert_eq!(
        written.len(),
        2,
        "旧的一张、新合的一张；这一批里没有的人不收"
    );
    assert_eq!(written[1].who, "qq:2");
    assert_eq!(written[1].text, "小林是群友，喜欢拍照。");
    assert_eq!(
        written[1].names,
        vec!["林林".to_string(), "小林".to_string(), "阿林".to_string()],
        "交回的在前，再接认出来的名字、旧卡上的"
    );

    // 又记了一条，整理交回同样的卡：不再记一张。
    person(&home, alice(), "qq:2", "小林下周去大阪", None);
    dream(&home).await.expect("整理了");
    assert_eq!(cards(&home).len(), 2, "和现在那一张一样的不记");
}

#[tokio::test]
async fn a_batch_without_people_has_no_card_part() {
    let server = Server::start(vec![stream("{}")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户养了一只猫");
    dream(&home).await.expect("整理了");
    assert!(!asked(&server, 0).contains(PEOPLE));
    assert!(cards(&home).is_empty());
}

/// 跟着看的补的时候（施工 E-1 下，`protocol.md`「记忆日志的推送」第 4 条）：整间清空以前的卡和作废不补，只清一个会话的不碍事，
/// 清空以后写的照补。
#[test]
fn catching_up_skips_cards_before_clearing_everything() {
    let home = Home::new();
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let card = |text: &str| {
        MemoryEvent::Card(Card {
            who: "qq:2".into(),
            text: text.into(),
            names: Vec::new(),
        })
    };
    let session = group_source(&home).session;
    for event in [
        card("清空以前的"),
        MemoryEvent::CardRetired(miyu_recall::CardRetired {
            who: "qq:2".into(),
            why: "认错了".into(),
        }),
        MemoryEvent::Cleared(miyu_recall::Cleared {
            session: Some(session),
        }),
    ] {
        log.append(now(), alice(), None, &event).expect("记得下");
    }
    assert_eq!(caught_up(&home), 2, "只清一个会话的照补");
    log.append(
        now(),
        alice(),
        None,
        &MemoryEvent::Cleared(miyu_recall::Cleared { session: None }),
    )
    .expect("记得下");
    log.append(now(), alice(), None, &card("清空以后的"))
        .expect("记得下");
    assert_eq!(
        cards(&home),
        vec![Card {
            who: "qq:2".into(),
            text: "清空以后的".into(),
            names: Vec::new(),
        }]
    );
    assert_eq!(caught_up(&home), 1, "整间清空以前的卡、作废都不补");
}

/// 补回来的人卡事件（卡和作废）有几条。
fn caught_up(home: &Home) -> usize {
    let following = keeper(home).follow(Some(0), 4096).expect("跟得上");
    following
        .filled
        .iter()
        .filter(|event| {
            matches!(
                from_event(event),
                Some(Ok(MemoryEvent::Card(_) | MemoryEvent::CardRetired(_)))
            )
        })
        .count()
}

//! 她不是她认识的人（施工 E-1 修，设计 42 第四节）：群里大家照人格的名字叫她，模型会写出 `who` 是她名字的认识的人。抽取、她当场
//! 记、整理认人都把这种改记成关于她自己的事（`episode`、不写是谁），不合她自己的卡；群友的群名片和她同名的，也不把她的事
//! 记到这个人头上、合进他的卡。抽取、整理记忆的模型是一台假服务器。

use std::sync::Arc;
use std::time::Duration;

use miyu_config::phrases::Label;
use miyu_http::testkit::Server;
use miyu_kernel::event::Event;
use miyu_kernel::id::{SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{Card, MemoryEvent, Saved, Source, from_event};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Dreamed, Keeper, Listener, NotDreamed, Stamp, Turn};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_tool::Remember;

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona};
use crate::support::*;

/// 合卡的那一段的开头（`resources/software/memory/merge/people.txt`）。
const PEOPLE: &str = "People you know in these memories:";

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

/// 这一间里她叫 Miyu。
fn named_miyu(home: &Home) {
    home.memory
        .name_her(&persona(), Some(&Label::One("Miyu".into())));
}

fn tool() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

/// 这一间记忆日志里记下的几条，照先后。
fn saved(home: &Home) -> Vec<Saved> {
    keeper(home)
        .follow(Some(0), 4096)
        .expect("跟得上")
        .filled
        .iter()
        .filter_map(|event| match from_event(event)?.ok()? {
            MemoryEvent::Saved(saved) => Some(saved),
            _ => None,
        })
        .collect()
}

fn cards(home: &Home) -> Vec<Card> {
    keeper(home)
        .follow(Some(0), 4096)
        .expect("跟得上")
        .filled
        .iter()
        .filter_map(|event| match from_event(event)?.ok()? {
            MemoryEvent::Card(card) => Some(card),
            _ => None,
        })
        .collect()
}

/// 旧主群那样的一个群会话：qq:2 以「小林」说了一句，qq:3888 把群名片改成了「Miyu」也说了一句。交回第 2 轮。
fn group_source(home: &Home) -> Source {
    let session = SessionId::parse("0192f3a0-3333-7abc-8def-001122334455").expect("合写法");
    let dir = home.root.session_dir(&session);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了日志");
    let line = |seq: u64, id: &str, name: &str, text: &str| {
        Event::from_line(&format!(
            r#"{{"seq":{seq},"at":"2026-10-22T03:00:00.000Z","kind":"message.user","by":{{"kind":"external","venue":"qq:group:5","id":"{id}"}},"body":{{"blocks":[{{"type":"text","text":"{text}"}}],"venue":{{"msg":"{seq}","name":"{name}"}}}}}}"#
        ))
        .expect("合写法")
    };
    log.append(&[
        line(1, "qq:2", "小林", "Miyu 是群管理"),
        line(2, "qq:3888", "Miyu", "我改名了"),
    ])
    .expect("写得进");
    Source {
        session,
        turn: TurnId::new(miyu_kernel::id::Seq::new(2).expect("从 1 起")),
    }
}

fn person(home: &Home, who: &str, text: &str, source: &Source) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "person".into(),
        text: text.into(),
        sources: vec![source.clone()],
        audience: vec![alice()],
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: Some(who.into()),
        people: Vec::new(),
        place: None,
    };
    log.append(now(), tool(), None, &MemoryEvent::Saved(saved))
        .expect("记得下");
}

async fn dream(home: &Home) -> Result<Dreamed, NotDreamed> {
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(home)
        .dream(config, UtcOffset::from_minutes(540).expect("合写法"))
        .await
}

/// 整理认人：她记的「Miyu」不认成改了群名片的那个群友，不合谁的卡，也不进谁的叫法；交回来的她自己的卡、那个群友的卡都不收。
#[tokio::test]
async fn a_dream_never_cards_her_or_gives_her_things_to_a_namesake() {
    let answer = r#"{"cards": [
        {"who": "qq:3888", "text": "他是群管理。"},
        {"who": "Miyu", "text": "Miyu 是群管理。"},
        {"who": "qq:2", "text": "小林是群友。", "names": ["林林"]}
    ]}"#;
    let server = Server::start(vec![stream(answer)]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    named_miyu(&home);
    let source = group_source(&home);
    person(&home, "Miyu", "Miyu 是群管理", &source);
    person(&home, "小林", "小林是群友", &source);
    person(&home, "qq:3888", "他改了群名片", &source);

    dream(&home).await.expect("整理了");
    let asked = asked(&server, 0);
    let row = asked
        .lines()
        .find(|line| line.contains("Miyu 是群管理"))
        .expect("这一条在批里");
    assert!(!row.contains("qq:3888"), "她的名字不认成那个群友：{row}");
    assert!(asked.contains(" person qq:2 "), "{asked}");
    assert!(
        asked.contains(&format!("{PEOPLE} qq:2 (小林), qq:3888.")),
        "她不在认识的人里，她的名字也不列成同名群友的名字（施工 E-1 修六）：{asked}"
    );
    let written = cards(&home);
    let whom: Vec<&str> = written.iter().map(|card| card.who.as_str()).collect();
    assert_eq!(whom, ["qq:3888", "qq:2"], "她自己的卡不收：{written:?}");
    for card in &written {
        assert!(
            !card.names.iter().any(|name| name == "Miyu"),
            "她的名字不进谁的叫法：{card:?}"
        );
    }
}

/// 她当场记一个认识的人、`who` 写的是她自己：记成关于她自己的事，不写是谁。
#[test]
fn remembering_herself_as_a_person_keeps_it_as_an_episode() {
    let home = Home::new();
    named_miyu(&home);
    let remember = |who: &str, text: &str| Remember {
        class: "person".into(),
        text: text.into(),
        replaces: None,
        keywords: None,
        who: Some(who.into()),
    };
    let stamp = || Stamp {
        at: now(),
        by: tool(),
        cause: None,
    };
    keeper(&home)
        .save(stamp(), remember(" miyu ", "Miyu 英语不好"), Vec::new())
        .expect("记得下");
    keeper(&home)
        .save(stamp(), remember("小林", "小林喜欢拍照"), Vec::new())
        .expect("记得下");
    let saved: Vec<(String, Option<String>)> = saved(&home)
        .into_iter()
        .map(|saved| (saved.class, saved.who))
        .collect();
    assert_eq!(
        saved,
        [
            ("episode".to_string(), None),
            ("person".to_string(), Some("小林".to_string()))
        ]
    );
}

/// 抽取：会话照人格的名字（样本的软件工程师，几种语言）记下她叫什么；模型写的 `who` 是她名字的记成 `episode`。
#[tokio::test]
async fn extraction_keeps_what_is_about_her_as_an_episode() {
    let memories: Vec<serde_json::Value> = (1..=TURNS)
        .flat_map(|turn| {
            [
                serde_json::json!({"class": "person", "text": "她英语不好", "turn": turn, "who": "Software Engineer"}),
                serde_json::json!({"class": "person", "text": "小林喜欢拍照", "turn": turn, "who": "小林"}),
            ]
        })
        .collect();
    let server = Server::start(vec![stream(
        &serde_json::json!({ "memories": memories }).to_string(),
    )])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let script = Script::new((0..2).map(|_| Play::Says("好。")));
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "你英语不好").await;
    chat(&home, &handle, 2, "小林喜欢拍照").await;
    requested(&server, 1).await;
    for _ in 0..100 {
        if mark(&home, handle.id()).is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let found = extracted(&home);
    assert!(!found.is_empty());
    for entry in &found {
        match entry.text.as_str() {
            "她英语不好" => assert_eq!(
                (entry.class.as_str(), entry.who.as_deref()),
                ("episode", None)
            ),
            _ => assert_eq!(
                (entry.class.as_str(), entry.who.as_deref()),
                ("person", Some("小林"))
            ),
        }
    }
    stop(&handle).await;
}

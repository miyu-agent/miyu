//! 她在场所里叫什么（施工 E-1 修三，设计 42 第四节）：群里大家叫她的未必是人格的名字（人格是「Software Engineer」、群名片是
//! 「米羽」），桥每条场所消息带 `venue.my_name`。她当场记、整理认人都把这个名字和人格的名字一起算她：`who` 是它的认识的人
//! 记成关于她自己的事，不合她的卡；群友的群名片也叫这个名字的，不把她的事记到这个人头上。抽取那一路在 `memory/her/tests.rs`。
//! 整理记忆的模型是一台假服务器；名字、编号都是编的。

use std::sync::Arc;

use miyu_config::phrases::Label;
use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Event, VenueMessage};
use miyu_kernel::id::{AccountId, ExternalId, SessionId, TurnId, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};
use miyu_kernel::session::Command;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{Card, MemoryEvent, Saved, Source, from_event};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Filter, Keeper, Listener, Turn};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::recall::Room;

use crate::support::extracting::*;
use crate::support::meaning::{catalog, persona};
use crate::support::*;

/// 合卡的那一段的开头（`resources/software/memory/merge/people.txt`）。
const PEOPLE: &str = "People you know in these memories:";

/// 群会话的记忆归的那个账号（同 `memory_listener.rs`）。
fn admin() -> AccountId {
    AccountId::parse("admin").expect("合写法")
}

fn place() -> VenueId {
    VenueId::parse("qq:group:7").expect("合写法")
}

/// 群里的小林（`qq:20017`）。
fn member() -> By {
    By::External(External {
        venue: place(),
        id: ExternalId::parse("qq:20017").expect("合写法"),
        account: None,
        role: Some(Role::Member),
    })
}

/// 小林在群里说 `words`、@ 了她，这时她在群里叫 `my_name`。
fn heard(words: &str, msg: &str, my_name: &str) -> Command {
    Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: Some(VenueMessage {
            msg: msg.to_string(),
            name: Some("小林".to_string()),
            my_name: Some(my_name.to_string()),
            mentions_me: true,
            ..VenueMessage::default()
        }),
    }
}

/// 她在群里叫「米羽」时当场记：`who` 写米羽的记成关于她自己的事、不写是谁；写小林的照旧是认识的人。
#[tokio::test]
async fn remembering_herself_by_her_name_in_the_group_keeps_it_as_an_episode() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[
            (
                "remember",
                r#"{"class":"person","text":"米羽是群管理","who":"米羽"}"#,
            ),
            (
                "remember",
                r#"{"class":"person","text":"小林喜欢拍照","who":"小林"}"#,
            ),
        ]),
        Play::Says("记住了。"),
    ]);
    let lines = Lines {
        venue: place(),
        group: true,
        memory_account: admin(),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    let mut pushes = watch(&handle).await;
    within(
        "回应",
        handle.command(
            id("cmd-1"),
            member(),
            heard("@米羽 你是群管理，记一下我喜欢拍照", "8801", "米羽"),
        ),
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let owner = Keeper::new(
        &home.memory,
        Room::persona(&admin(), "engineer"),
        Listener::Owner(By::Person(Person::new(admin()))),
    );
    let mut found: Vec<(String, String, Option<String>)> = owner
        .list(&Filter {
            limit: 50,
            ..Filter::default()
        })
        .expect("列得出")
        .into_iter()
        .map(|entry| (entry.text, entry.class, entry.who))
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            (
                "小林喜欢拍照".to_string(),
                "person".to_string(),
                Some("小林".to_string())
            ),
            ("米羽是群管理".to_string(), "episode".to_string(), None),
        ]
    );
    stop(&handle).await;
}

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

fn tool() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

/// 一个群会话：qq:2 以「小林」说了一句、那时她在群里叫「米羽」；qq:3888 的群名片也叫「米羽」。交回第 2 轮。
fn group_source(home: &Home) -> Source {
    let session = SessionId::parse("0192f3a0-4444-7abc-8def-001122334455").expect("合写法");
    let dir = home.root.session_dir(&alice_account(), &session);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了日志");
    let line = |seq: u64, id: &str, name: &str, my_name: &str, text: &str| {
        Event::from_line(&format!(
            r#"{{"seq":{seq},"at":"2026-10-22T03:00:00.000Z","kind":"message.user","by":{{"kind":"external","venue":"qq:group:5","id":"{id}"}},"body":{{"blocks":[{{"type":"text","text":"{text}"}}],"venue":{{"msg":"{seq}","name":"{name}","my_name":"{my_name}"}}}}}}"#
        ))
        .expect("合写法")
    };
    log.append(&[
        line(1, "qq:2", "小林", "米羽", "米羽是群管理"),
        line(2, "qq:3888", "米羽", "米羽", "我也叫米羽"),
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
    };
    log.append(now(), tool(), None, &MemoryEvent::Saved(saved))
        .expect("记得下");
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

/// 整理认人照出处那几轮的会话日志里她在群里叫什么：她记的「米羽」不认成同名的群友，不合谁的卡，也不进谁的叫法；交回的米羽的
/// 卡不收。人格叫 Miyu，和「米羽」不同。
#[tokio::test]
async fn a_dream_counts_her_name_in_the_venue_as_her() {
    let answer = r#"{"cards": [
        {"who": "米羽", "text": "米羽是群管理。"},
        {"who": "qq:2", "text": "小林是群友。"}
    ]}"#;
    let server = Server::start(vec![stream(answer)]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    home.memory
        .name_her(&persona(), Some(&Label::One("Miyu".into())));
    let source = group_source(&home);
    person(&home, "米羽", "米羽是群管理", &source);
    person(&home, "小林", "小林是群友", &source);

    let config_source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(config_source.with_project("~"), config_source));
    keeper(&home)
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await
        .expect("整理了");
    let asked = asked(&server, 0);
    let row = asked
        .lines()
        .find(|line| line.contains("米羽是群管理"))
        .expect("这一条在批里");
    assert!(
        !row.contains("qq:3888"),
        "她在群里的名字不认成同名群友：{row}"
    );
    assert!(
        asked.contains(&format!("{PEOPLE} qq:2.")),
        "她不在认识的人里：{asked}"
    );
    let written = cards(&home);
    let whom: Vec<&str> = written.iter().map(|card| card.who.as_str()).collect();
    assert_eq!(whom, ["qq:2"], "她的卡不收：{written:?}");
    assert!(
        written
            .iter()
            .all(|card| !card.names.iter().any(|name| name == "米羽")),
        "她的名字不进谁的叫法：{written:?}"
    );
}

/// 抽取照这一段里她在群里被叫过的名字认她：模型写的 `who` 是米羽的记成关于她自己的事；小林照旧认成他的平台编号。
#[tokio::test]
async fn extraction_counts_her_name_in_the_group_as_her() {
    let memories: Vec<serde_json::Value> = (1..=TURNS)
        .flat_map(|turn| {
            [
                serde_json::json!({"class": "person", "text": "米羽是群管理", "turn": turn, "who": "米羽"}),
                serde_json::json!({"class": "person", "text": "小林喜欢拍照", "turn": turn, "who": "小林"}),
            ]
        })
        .collect();
    let server = Server::start(vec![stream(
        &serde_json::json!({ "memories": memories }).to_string(),
    )])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 2\n");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let lines = Lines {
        venue: place(),
        group: true,
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    // 头一轮她叫米羽，第二轮她已经改叫 MiyuBot：两轮一起抽，这一段里出现过的都算她，不只最近的那个。
    for (n, words, my_name) in [
        (1, "@米羽 你是群管理", "米羽"),
        (2, "@MiyuBot 我喜欢拍照", "MiyuBot"),
    ] {
        let mut pushes = watch(&handle).await;
        within(
            "回应",
            handle.command(
                id(&format!("cmd-{n}")),
                member(),
                heard(words, &format!("880{n}"), my_name),
            ),
        )
        .await
        .expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    requested(&server, 1).await;
    // 候选一条条写进记忆日志，抽到了哪的记号最后写：等记号出现再读，读到不空不等于写完了。
    for _ in 0..200 {
        if mark(&home, handle.id()).is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let mut placed: Vec<(String, String, Option<String>)> = extracted(&home)
        .into_iter()
        .map(|entry| (entry.text, entry.class, entry.who))
        .collect();
    placed.sort();
    placed.dedup();
    assert_eq!(
        placed,
        [
            (
                "小林喜欢拍照".to_string(),
                "person".to_string(),
                Some("qq:20017".to_string())
            ),
            ("米羽是群管理".to_string(), "episode".to_string(), None),
        ]
    );
    stop(&handle).await;
}

/// 载入以后照样认得她在群里叫什么（施工 E-1 修三补）：头一轮小林叫她「米羽」；会话停了再载入，这一句没带她叫什么，她当场
/// 记 `who` 是米羽的照样记成关于她自己的事。原来载入时只把换人格的几条交给记忆，这里接不上，记成了认识的人。
#[tokio::test]
async fn after_a_load_she_still_knows_what_the_group_calls_her() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("在。"),
        Play::calls(&[(
            "remember",
            r#"{"class":"person","text":"米羽是群管理","who":"米羽"}"#,
        )]),
        Play::Says("记住了。"),
    ]);
    let lines = Lines {
        venue: place(),
        group: true,
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    let session = handle.id().clone();
    let mut pushes = watch(&handle).await;
    within(
        "回应",
        handle.command(id("cmd-1"), member(), heard("@米羽 在吗", "8801", "米羽")),
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;

    let handle = home.load_with(&session, &script, &catalog(&home)).await;
    let mut unnamed = heard("你是群管理，记一下", "8802", "米羽");
    if let Command::Send {
        venue: Some(venue), ..
    } = &mut unnamed
    {
        venue.my_name = None;
    }
    let mut pushes = watch(&handle).await;
    within("回应", handle.command(id("cmd-2"), member(), unnamed))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    let found: Vec<(String, String, Option<String>)> = keeper(&home)
        .list(&Filter {
            limit: 50,
            ..Filter::default()
        })
        .expect("列得出")
        .into_iter()
        .map(|entry| (entry.text, entry.class, entry.who))
        .collect();
    assert_eq!(
        found,
        [("米羽是群管理".to_string(), "episode".to_string(), None)]
    );
}

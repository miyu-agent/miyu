//! 发生过的事涉及谁（施工 E-1 修四）：抽取交回的 `people` 照这一段的说话人认、她自己和主人不算，记进 `ext.memory.saved`；改一条
//! 发生过的事照旧带着涉及谁，改成别的类的不带。整理时涉及谁的那几条算进这个人：请求里标着 `episode with <是谁>`，列进这一批的
//! 人，交回他的卡照收。抽取、整理记忆的模型是一台假服务器。

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::event::Event;
use miyu_kernel::id::{SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{Card, EPISODE, MemoryEvent, MemoryId, Saved, Source, from_event};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Keeper, Listener, Stamp, Turn};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_tool::Remember;

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona};
use crate::support::*;

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
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

/// 抽取：发生过的事交回的涉及谁里，她自己（样本的软件工程师）、主人不算，对不上谁的照名字留；记下来的那一条带着。
#[tokio::test]
async fn an_extracted_episode_keeps_who_it_involves() {
    let memories: Vec<serde_json::Value> = (1..=TURNS)
        .map(|turn| {
            serde_json::json!({"class": "episode", "text": "小林把她 P 成了猪", "turn": turn,
                "people": ["小林", "Software Engineer", "Owner", "小林"]})
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
    chat(&home, &handle, 1, "小林把你 P 成了猪").await;
    chat(&home, &handle, 2, "哈哈").await;
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
        assert_eq!(
            (entry.class.as_str(), entry.people.as_slice()),
            (EPISODE, &["小林".to_string()][..])
        );
    }
    let logged = saved(&home);
    assert!(
        logged.iter().all(|saved| saved.people == ["小林"]),
        "记进 ext.memory.saved"
    );
    stop(&handle).await;
}

/// 改一条发生过的事（她的工具不收涉及谁）照旧带着涉及谁；改成别的类的不带。
#[test]
fn revising_an_episode_keeps_who_it_involves() {
    let home = Home::new();
    let (log, _) = home.memory.logs.open(&persona()).expect("开得了");
    let first = Saved {
        class: EPISODE.into(),
        text: "小林把她 P 成了猪".into(),
        sources: Vec::new(),
        audience: vec![alice()],
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: None,
        people: vec!["qq:2".into()],
    };
    let id = log
        .append(now(), alice(), None, &MemoryEvent::Saved(first))
        .expect("记得下")
        .id;
    let stamp = || Stamp {
        at: now(),
        by: alice(),
        cause: None,
    };
    let revise = |class: &str, text: &str, replaces: MemoryId| Remember {
        class: class.into(),
        text: text.into(),
        replaces: Some(replaces),
        keywords: None,
        who: None,
    };
    let second = keeper(&home)
        .save(
            stamp(),
            revise(EPISODE, "小林把她 P 成了一只猪", id),
            Vec::new(),
        )
        .expect("改得了");
    keeper(&home)
        .save(stamp(), revise("user", "主人也笑了", second), Vec::new())
        .expect("改得了");
    let people: Vec<Vec<String>> = saved(&home).into_iter().map(|saved| saved.people).collect();
    assert_eq!(
        people,
        [
            vec!["qq:2".to_string()],
            vec!["qq:2".to_string()],
            Vec::new()
        ]
    );
}

/// 记一条发生过的事：`by` 写它的那一方，涉及 `people`，出处是 `source` 的话。
fn episode(home: &Home, by: By, text: &str, people: &[&str], source: Option<Source>) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: EPISODE.into(),
        text: text.into(),
        sources: source.into_iter().collect(),
        audience: vec![alice()],
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: None,
        people: people.iter().map(|name| name.to_string()).collect(),
    };
    log.append(now(), by, None, &MemoryEvent::Saved(saved))
        .expect("记得下");
}

/// 群里的一个会话：qq:2 以群名片「小林」说了一句（第 2 轮）。交回那一轮。
fn group_source(home: &Home) -> Source {
    let session = SessionId::parse("0192f3a0-3333-7abc-8def-001122334455").expect("合写法");
    let dir = home.root.session_dir(&alice_account(), &session);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了日志");
    let said = Event::from_line(
        r#"{"seq":1,"at":"2026-10-22T03:00:00.000Z","kind":"message.user","by":{"kind":"external","venue":"qq:group:5","id":"qq:2"},"body":{"blocks":[{"type":"text","text":"我把你 P 成猪了"}],"venue":{"msg":"9","name":"小林"}}}"#,
    )
    .expect("合写法");
    log.append(&[said]).expect("写得进");
    Source {
        session,
        turn: TurnId::new(miyu_kernel::id::Seq::new(2).expect("从 1 起")),
    }
}

/// 这一间记忆日志里的人卡事件，照先后。
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

/// 整理：涉及谁的发生过的事算进这个人。她当场记的写的是群名片，照出处的会话日志认成平台编号；抽取记的本来就是平台编号；
/// 不涉及谁的照旧只写类。这几个人列进这一批，交回他们的卡照收，叫法照出处里真用过的。
#[tokio::test]
async fn a_dream_cards_the_people_an_episode_involves() {
    let answer = r#"{"cards": [
        {"who": "qq:2", "text": "小林爱开玩笑，把你 P 成过猪。"},
        {"who": "qq:3", "text": "阿杰被你禁言过一次。", "names": ["阿杰", "杰哥"]}
    ]}"#;
    let server = Server::start(vec![stream(answer)]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let source = group_source(&home);
    let tool: By = serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法");
    episode(&home, tool, "小林把她 P 成了猪", &["小林"], Some(source));
    episode(&home, alice(), "阿杰被她禁言了十分钟", &["qq:3"], None);
    episode(&home, alice(), "群里停电了", &[], None);
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(&home)
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await
        .expect("整理了");
    let asked = asked(&server, 0);
    assert!(
        asked.contains(" episode with qq:2 "),
        "她记的名字照出处认：{asked}"
    );
    assert!(asked.contains(" episode with qq:3 "), "{asked}");
    assert!(
        asked.contains(" episode 2026-"),
        "不涉及谁的照旧只写类：{asked}"
    );
    assert!(
        asked.contains("People you know in these memories: qq:2, qq:3."),
        "涉及的人列进这一批：{asked}"
    );
    assert!(
        asked.contains(r#"an episode "with" them included"#),
        "合卡的那一段写明涉及他的也算他的：{asked}"
    );
    let written = cards(&home);
    let who: Vec<&str> = written.iter().map(|card| card.who.as_str()).collect();
    assert_eq!(who, ["qq:2", "qq:3"], "只在发生过的事里出现的人也合卡");
    assert_eq!(written[0].names, ["小林"], "叫法照出处里真用过的");
    assert_eq!(
        written[1].names,
        ["阿杰"],
        "交回的叫法写在涉及他的那条正文里的算见过，编的不要"
    );
}

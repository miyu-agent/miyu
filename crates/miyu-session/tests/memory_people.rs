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
        place: None,
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
        place: None,
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
        asked.contains("People you know in these memories: qq:2 (小林), qq:3."),
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

/// 群里的一个会话：qq:2 以「小林」、qq:5 以「老周」各说了一句（第 2 轮）。交回那一轮。
fn two_speaking(home: &Home) -> Source {
    let session = SessionId::parse("0192f3a0-4444-7abc-8def-001122334455").expect("合写法");
    let dir = home.root.session_dir(&alice_account(), &session);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了日志");
    let lines = [
        r#"{"seq":1,"at":"2026-10-22T03:00:00.000Z","kind":"message.user","by":{"kind":"external","venue":"qq:group:5","id":"qq:2"},"body":{"blocks":[{"type":"text","text":"帮我看看老周说的那个框架"}],"venue":{"msg":"9","name":"小林"}}}"#,
        r#"{"seq":2,"at":"2026-10-22T03:00:01.000Z","kind":"message.user","by":{"kind":"external","venue":"qq:group:5","id":"qq:5"},"body":{"blocks":[{"type":"text","text":"就是那个 nui"}],"venue":{"msg":"10","name":"老周"}}}"#,
    ];
    let said: Vec<Event> = lines
        .iter()
        .map(|line| Event::from_line(line).expect("合写法"))
        .collect();
    log.append(&said).expect("写得进");
    Source {
        session,
        turn: TurnId::new(miyu_kernel::id::Seq::new(2).expect("从 1 起")),
    }
}

/// 整理（施工 E-1 修五）：模型把正文里提到的别人的名字也交进了卡的叫法。带着两个人的发生过的事，正文不拿来认叫法（小林那张
/// 不收「阿杰」，阿杰那张也不收）；只涉及一个人的照旧认，但别人在出处里用过的名片不算（老周那张不收「小林」）。
#[tokio::test]
async fn names_of_two_people_in_one_episode_do_not_cross() {
    let answer = r#"{"cards": [
        {"who": "qq:2", "text": "小林让你看过一个框架。", "names": ["小林", "阿杰"]},
        {"who": "qq:5", "text": "老周嫌别人代码乱。", "names": ["老周", "小林"]},
        {"who": "qq:7", "text": "阿杰提过一个框架。", "names": ["阿杰"]}
    ]}"#;
    let server = Server::start(vec![stream(answer)]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let source = two_speaking(&home);
    episode(
        &home,
        alice(),
        "应小林要求，她看了阿杰提的 nui 框架",
        &["qq:2", "qq:7"],
        Some(source.clone()),
    );
    episode(
        &home,
        alice(),
        "老周嫌小林的代码太乱",
        &["qq:5"],
        Some(source),
    );
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
    assert!(
        asked(&server, 0)
            .contains("People you know in these memories: qq:2 (小林), qq:5 (老周), qq:7."),
        "列人时各接各在出处里用过的名字，不互串（施工 E-1 修六）：{}",
        asked(&server, 0)
    );
    let written = cards(&home);
    let who: Vec<&str> = written.iter().map(|card| card.who.as_str()).collect();
    assert_eq!(who, ["qq:2", "qq:5", "qq:7"], "三个人都合卡");
    assert_eq!(
        written[0].names,
        ["小林"],
        "带着两个人的那条正文里的阿杰不进小林的卡"
    );
    assert_eq!(
        written[1].names,
        ["老周"],
        "小林在出处里用过的名片不进老周的卡"
    );
    assert!(
        written[2].names.is_empty(),
        "带着两个人的那条正文不拿来认阿杰的叫法：{:?}",
        written[2].names
    );
}

/// 群里的一个会话：`id` 照先后用 `names` 里的名字各说一句。交回最后一轮。
fn renamed(home: &Home, session: &str, id: &str, names: &[&str]) -> Source {
    let session = SessionId::parse(session).expect("合写法");
    let dir = home.root.session_dir(&alice_account(), &session);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了日志");
    let said: Vec<Event> = names
        .iter()
        .enumerate()
        .map(|(n, name)| {
            let line = serde_json::json!({
                "seq": n + 1,
                "at": format!("2026-10-22T03:00:{n:02}.000Z"),
                "kind": "message.user",
                "by": {"kind": "external", "venue": "qq:group:5", "id": id},
                "body": {"blocks": [{"type": "text", "text": "在吗"}], "venue": {"msg": format!("{n}"), "name": name}},
            });
            Event::from_line(&line.to_string()).expect("合写法")
        })
        .collect();
    log.append(&said).expect("写得进");
    Source {
        session,
        turn: TurnId::new(miyu_kernel::id::Seq::new(names.len() as u64).expect("从 1 起")),
    }
}

/// 整理（施工 E-1 修六）：合卡那一段列人时，每个人后面括号里接他在出处里用过的名字，最近用的在前、照模板的规矩转义、最多
/// 八个；出处里没用过名字的只写是谁。指令写明名片里多出来的话是名字的一部分。
#[tokio::test]
async fn a_dream_lists_the_names_each_person_used() {
    let server = Server::start(vec![stream(r#"{"cards": []}"#)]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let tagged = renamed(
        &home,
        "0192f3a0-5555-7abc-8def-001122334455",
        "qq:2",
        &["小林", r#"我不是Miyu，叫"米鱼"<b>&"#],
    );
    episode(
        &home,
        alice(),
        "小林说自己学临床医学",
        &["qq:2"],
        Some(tagged),
    );
    let many: Vec<String> = (0..10).map(|n| format!("名{n}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    let busy = renamed(&home, "0192f3a0-6666-7abc-8def-001122334455", "qq:9", &many);
    episode(&home, alice(), "名9 天天改名片", &["qq:9"], Some(busy));
    episode(&home, alice(), "阿杰被她禁言了十分钟", &["qq:3"], None);
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
    let line = asked
        .lines()
        .find(|line| line.starts_with("People you know in these memories:"))
        .expect("有合卡那一段");
    let listed = line
        .strip_prefix("People you know in these memories: ")
        .and_then(|rest| rest.split(". Names in the memories").next())
        .expect("列人接着那一句");
    assert_eq!(
        listed,
        r"qq:2 (我不是Miyu，叫\u0022米鱼\u0022\u003cb\u003e\u0026, 小林), qq:3, qq:9 (名9, 名8, 名7, 名6, 名5, 名4, 名3, 名2)",
        "最近用的在前、转义、最多八个，没用过名字的只写是谁"
    );
    assert!(
        line.contains("extra words in a nickname (a joke, a note, a claim like \"not X\") are part of the name, never a fact about them."),
        "指令写明名片里多出来的话是名字的一部分：{line}"
    );
}

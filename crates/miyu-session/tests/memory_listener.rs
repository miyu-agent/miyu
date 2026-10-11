//! 谁在听（施工 R-13 上，`docs/blueprint/memory.md` 第九条）：记忆只有一份，就像一个人只有一个大脑；主人那边冒得出全部，一个
//! 场所里只冒出在这个场所说过的。场所里记的带着这个场所；场所里的常驻那一块没有摘要那一行；合并照全部合，不管是哪个会话抽完
//! 以后起的。

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::VenueMessage;
use miyu_kernel::id::{AccountId, ExternalId, Seq, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_recall::{MemoryEvent, MemoryId, Saved, Summary};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Filter, Keeper, Listener, Stamp};
use miyu_store::recall::Room;
use miyu_tool::{Refused, Remember};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

/// 记忆归的那个账号：会话的属主是 alice（夹具定的），记忆归 admin 的会话就是「属主不是记忆归的那个账号」的场所会话。
fn admin() -> AccountId {
    AccountId::parse("admin").expect("合写法")
}

fn room() -> Room {
    Room::persona(&admin(), "engineer")
}

fn place(n: u32) -> VenueId {
    VenueId::parse(&format!("qq:group:{n}")).expect("合写法")
}

fn owner() -> Listener {
    Listener::Owner(By::Person(Person::new(admin())))
}

/// 在这一间记一条：`venue` 是在哪个场所说的。
fn save(home: &Home, text: &str, venue: Option<VenueId>) -> MemoryId {
    let (log, _) = home.logs.open(&room()).expect("开得了");
    let saved = Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: Vec::new(),
        replaces: None,
        about: None,
        venue,
    };
    log.append(now(), alice(), None, &MemoryEvent::Saved(saved))
        .expect("记得下")
        .id
}

fn texts(keeper: &Keeper) -> Vec<String> {
    let filter = Filter {
        limit: 50,
        ..Filter::default()
    };
    let mut texts: Vec<String> = keeper
        .list(&filter)
        .expect("列得出")
        .into_iter()
        .map(|entry| entry.text)
        .collect();
    texts.sort();
    texts
}

fn stamp() -> Stamp {
    Stamp {
        at: now(),
        by: alice(),
        cause: None,
    }
}

#[test]
fn whoever_the_memory_belongs_to_is_the_owner_and_anyone_else_is_a_place() {
    let local = VenueId::parse("local").expect("合写法");
    assert_eq!(Listener::of(&admin(), &admin(), &local), owner());
    assert_eq!(
        Listener::of(&admin(), &admin(), &place(1)),
        owner(),
        "主人自己的私聊"
    );
    let onebot = AccountId::parse("onebot").expect("合写法");
    assert_eq!(
        Listener::of(&admin(), &onebot, &place(1)),
        Listener::Place(place(1))
    );
}

#[tokio::test]
async fn the_owner_recalls_everything_and_a_place_only_its_own() {
    let home = Home::new();
    let mine = save(&home, "主人养了一只猫", None);
    let theirs = save(&home, "小林在学日语", Some(place(1)));
    save(&home, "二群在办读书会", Some(place(2)));
    let owner = Keeper::new(&home.memory, room(), owner());
    let first = Keeper::new(&home.memory, room(), Listener::Place(place(1)));
    assert_eq!(
        texts(&owner),
        ["主人养了一只猫", "二群在办读书会", "小林在学日语"],
        "一个大脑：主人那边全部"
    );
    assert_eq!(
        texts(&first),
        ["小林在学日语"],
        "这个场所只冒出在这里说过的"
    );
    let found = |keeper: &Keeper, query: &str| {
        keeper
            .search(query, false, 10, None)
            .expect("搜得了")
            .into_iter()
            .map(|entry| entry.text)
            .collect::<Vec<String>>()
    };
    assert_eq!(found(&first, "日语"), ["小林在学日语"]);
    assert!(found(&first, "猫").is_empty(), "主人的事在群里冒不出来");
    assert!(
        matches!(
            first.retire(stamp(), mine, "不对".into()),
            Err(Refused::NoSuch(_))
        ),
        "冒不出来的当没有"
    );

    let remember = Remember {
        class: "user".into(),
        text: "小林下周考试".into(),
        replaces: None,
    };
    let id = first.save(stamp(), remember, Vec::new()).expect("记得下");
    let (log, _) = home.logs.open(&room()).expect("开得了");
    let entry = log.book(|book| book.get(id).cloned()).expect("有");
    assert_eq!(entry.venue, Some(place(1)), "场所里记的带着这个场所");
    assert!(entry.audience.is_empty());
    owner
        .retire(stamp(), theirs, "考完了".into())
        .expect("主人那边碰得到群里的");
}

/// 请求里人那一边的字块。
fn said(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.clone(),
            _ => Vec::new(),
        })
        .filter_map(|block| match block {
            Block::Text(Text { text }) => Some(text),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_place_gets_only_its_own_lines_and_no_digest() {
    let home = Home::new();
    let mine = save(&home, "主人养了一只猫", None);
    save(&home, "小林在学日语", Some(place(1)));
    let (log, _) = home.logs.open(&room()).expect("开得了");
    let digest = MemoryEvent::Summary(Summary {
        text: "主人的私事".into(),
        upto: mine.seq(),
    });
    log.append(now(), alice(), None, &digest).expect("记得下");
    let script = Script::new([Play::Says("好。")]);
    let lines = Lines {
        venue: place(1),
        memory_account: admin(),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    chat(&home, &handle, 1, "在吗").await;
    let block = said(&script.requests()[0].1)
        .into_iter()
        .find(|text| text.starts_with("<memories>"))
        .expect("有那一块");
    assert!(block.contains("小林在学日语"), "{block}");
    assert!(
        !block.contains("主人"),
        "没有摘要那一行、没有主人的事：{block}"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn a_merge_started_in_a_place_sees_everything() {
    let server = Server::start(vec![
        every_turn("小林在学日语"),
        stream(r#"{"revised": [], "retired": [], "summary": "主人养了一只猫；小林在学日语。"}"#),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 1\n");
    save(&home, "主人养了一只猫", None);
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let lines = Lines {
        venue: place(1),
        memory_account: admin(),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    chat(&home, &handle, 1, "我在学日语").await;
    chat(&home, &handle, 2, "下周考试").await;
    requested(&server, 2).await;
    let text = asked(&server, 1);
    assert!(text.contains("主人养了一只猫"), "合并照全部：{text}");
    stop(&handle).await;
    let _ = Seq::new(1);
}

/// 群里的小林（`qq:20017`）。
fn member() -> By {
    By::External(External {
        venue: place(1),
        id: ExternalId::parse("qq:20017").expect("合写法"),
        account: None,
        role: Some(Role::Member),
    })
}

/// 小林在群里说 `words`（平台的编号 `msg`）；`ambient` 的是旁听，不开回合。
fn heard(words: &str, msg: &str, ambient: bool) -> Command {
    Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: Some(VenueMessage {
            msg: msg.to_string(),
            name: Some("小林".to_string()),
            ambient,
            ..VenueMessage::default()
        }),
    }
}

#[tokio::test]
async fn in_a_group_what_she_extracts_says_who_spoke_and_stays_in_that_group() {
    let server = Server::start(vec![every_turn("小林在学日语")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 1\n");
    let script = Script::new([Play::Says("加油。")]);
    let lines = Lines {
        venue: place(1),
        group: true,
        memory_account: admin(),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    within(
        "回应",
        handle.command(id("cmd-1"), member(), heard("早", "8800", true)),
    )
    .await
    .expect("会话在跑");
    let mut pushes = watch(&handle).await;
    within(
        "回应",
        handle.command(id("cmd-2"), member(), heard("我在学日语", "8801", false)),
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    requested(&server, 1).await;
    let text = asked(&server, 0);
    assert!(
        text.lines()
            .any(|line| line.starts_with('[') && line.contains("小林 [msg=8801]: 我在学日语")),
        "群里的一行原样，前面不另加谁说的：{text}"
    );
    assert!(text.contains("You: 加油。"), "{text}");
    assert!(!text.contains("Owner:"), "群里没人被当成主人：{text}");

    let owner = Keeper::new(&home.memory, room(), owner());
    let mut found = Vec::new();
    for _ in 0..200 {
        found = owner
            .list(&Filter {
                limit: 50,
                ..Filter::default()
            })
            .expect("列得出");
        if !found.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let entry = found.first().expect("记下了");
    assert_eq!(entry.text, "小林在学日语");
    assert_eq!(entry.venue, Some(place(1)), "记下的带着这个群");
    let elsewhere = Keeper::new(&home.memory, room(), Listener::Place(place(2)));
    assert!(texts(&elsewhere).is_empty(), "别的群里冒不出来");
    stop(&handle).await;
}

//! 谁在听（施工 R-13 上，`docs/blueprint/memory.md` 第九条）：记忆只有一份，就像一个人只有一个大脑；主人那边冒得出全部，一个
//! 场所里只冒出在这个场所说过的。场所里记的带着这个场所；场所里的常驻那一块没有摘要那一行；合并照全部合，不管是哪个会话抽完
//! 以后起的；群里的抽取只列在这个群里忘掉的（施工 R-6 修）。

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::VenueMessage;
use miyu_kernel::id::{AccountId, ExternalId, Seq, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_recall::{MemoryEvent, MemoryId, Retired, Saved, Summary};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Filter, Keeper, Listener, Stamp};
use miyu_store::recall::Room;
use miyu_tool::Remember;

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
        keywords: None,
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
async fn everywhere_recalls_everything() {
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
        texts(&owner),
        "记忆是一个整体：场所里也是全部（施工 R-29）"
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
    assert_eq!(found(&first, "猫"), ["主人养了一只猫"], "主人的事群里也想得起");
    assert_eq!(found(&first, "读书会"), ["二群在办读书会"], "别的群的也是");
    first
        .retire(stamp(), mine, "不对".into())
        .expect("群里也碰得到主人那边记的");

    let remember = Remember {
        class: "user".into(),
        text: "小林下周考试".into(),
        replaces: None,
        keywords: None,
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
async fn a_place_gets_the_digest_and_every_line() {
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
        block.contains("主人的私事"),
        "摘要那一行哪儿都交（施工 R-29）：{block}"
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
    assert_eq!(texts(&elsewhere), ["小林在学日语"], "别的群里也想得起（施工 R-29）");
    stop(&handle).await;
}

/// 以前的对话哪儿都搜得到全部（施工 R-29；R-13 下到 R-29 之间照场所挑）：两个群各说一轮，一群里、主人那边都两个都有。
#[tokio::test]
async fn past_turns_from_any_place_are_found_anywhere() {
    let home = Home::new();
    let mut sessions = Vec::new();
    for n in [1, 2] {
        let script = Script::new([Play::Says("好。")]);
        let lines = Lines {
            venue: place(n),
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
                heard(&format!("我在{n}群学日语"), "8801", false),
            ),
        )
        .await
        .expect("会话在跑");
        until_turn_ends(&mut pushes).await;
        sessions.push(handle.id().clone());
        stop(&handle).await;
    }
    let found = |listener: Listener| {
        let mut sessions: Vec<String> = Keeper::new(&home.memory, room(), listener)
            .turns("日语", 10, None, None)
            .expect("搜得了")
            .into_iter()
            .map(|turn| turn.session.to_string())
            .collect();
        sessions.sort();
        sessions
    };
    let mut both: Vec<String> = sessions.iter().map(ToString::to_string).collect();
    both.sort();
    assert_eq!(found(Listener::Place(place(1))), both, "一群里也搜得到二群的");
    assert_eq!(found(owner()), both, "主人那边全部");
}

/// 群里那一段开始以后忘掉的，抽取全列出来（施工 R-6 修，`memory.md` 第六条第 4 款；R-29 起不照场所挑）：主人那边作废的也在。
#[tokio::test]
async fn in_a_group_everything_dropped_follows_the_slice() {
    let server = Server::start(vec![every_turn("不要紧")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 1\n");
    let private = save(&home, "主人下周三去体检", None);
    let shared = save(&home, "群里周六聚餐", Some(place(1)));
    let script = Script::new([Play::Says("好。")]);
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
    let (log, _) = home.logs.open(&room()).expect("开得了");
    for id in [private, shared] {
        let retired = Retired {
            id,
            why: "不算了".into(),
            kind: None,
            kept: None,
        };
        log.append(now(), alice(), None, &MemoryEvent::Retired(retired))
            .expect("记得下");
    }
    let mut pushes = watch(&handle).await;
    within(
        "回应",
        handle.command(id("cmd-2"), member(), heard("聚餐还办吗", "8801", false)),
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    requested(&server, 1).await;
    let text = asked(&server, 0);
    assert!(
        text.contains("群里周六聚餐"),
        "这个群里忘掉的列出来：{text}"
    );
    assert!(
        text.contains("主人下周三去体检"),
        "主人那边作废的也列出来（施工 R-29）：{text}"
    );
    stop(&handle).await;
}

/// 群会话写了群聊近况最多几条（施工 O-13 再补）：抽取每一轮也只带那么多句，和她当时看到的一样。
#[tokio::test]
async fn a_counted_group_extracts_only_that_many_lines_per_round() {
    let server = Server::start(vec![every_turn("小林在学日语")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 1\n");
    let script = Script::new([Play::Says("加油。")]);
    let lines = Lines {
        venue: place(1),
        group: true,
        recent: Some(1),
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
    assert!(text.contains("[msg=8801]: 我在学日语"), "{text}");
    assert!(!text.contains("[msg=8800]"), "只带最近一句：{text}");
    stop(&handle).await;
}

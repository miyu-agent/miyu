//! 以前的对话打不开的不给编号（施工 E-6，`docs/blueprint/memory.md`「工具」）：她拿 `memory_search` 搜到的会话编号去 `history`
//! 读整段，`history` 只认属主一样的主会话（`SessionsPort::list`）。回合库记下每个会话的属主、是不是主会话（`origins`），搜的
//! 时候照它判：打得开的照旧给编号，分叉出来的支线、别的账号的、场所会话（属主是桥的系统账号）不给编号、写在哪聊的。真会话、
//! 真回合库，执行器替身照剧本调工具；名字、群号是编的。

use std::sync::Arc;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, CommandId, VenueId};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Fork, Handle, Keeper, Listener, SessionPort, new_id};
use miyu_store::recall::{Edit, Origin, Room};
use miyu_tool::Catalog;

use crate::support::table::Table;
use crate::support::*;

/// 基础系统加记忆的三件，和核心里一样。
fn catalog(home: &Home) -> Catalog {
    let mut tools = miyu_basesystem::tools(home.resources.path()).expect("读得出");
    tools.extend(miyu_memory::tools(home.resources.path()).expect("读得出"));
    Catalog::new(tools).expect("合写法")
}

fn room() -> Room {
    Room::persona(&alice_account(), "engineer")
}

/// 说一句，等这一轮结束。
async fn chat(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里最后一次调用交回的字。
fn last_result(log: &[Event]) -> String {
    log.iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text, .. }) => text.clone(),
                        other => format!("{other:?}"),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .expect("有一次调用")
}

/// 主线、分叉出来的支线各说一轮，新开的本机会话里搜：主线那一段带编号（`history` 读得到），支线那一段不带、什么场所都不加
/// （本机的）。回合库记下两个会话的属主和是不是主会话。
#[tokio::test]
async fn a_lane_found_in_another_session_has_no_id() {
    let home = Home::new();
    let tools = catalog(&home);
    let talk = Script::new([Play::Says("好。"), Play::Says("加油。")]);
    // 先开一个会话：这一间的回合库开了、补完旧会话，后面新造的只靠头一回写时记下从哪来。
    let warm = home
        .create_with(&Script::new([Play::Says("在。")]), &tools)
        .await;
    chat(&warm, "c0", "在吗").await;
    home.memory.settle();
    stop(&warm).await;
    let parent = home.create_with(&talk, &tools).await;
    chat(&parent, "c1", "我在学日语").await;
    // 新造的会话接上时还没开库：头一回写回合库时记下从哪来，不等下次载入、补旧会话。
    home.memory.settle();
    assert_eq!(
        home.recall
            .turns(&room())
            .0
            .origins()
            .expect("读得了")
            .get(parent.id().as_str()),
        Some(&Origin {
            owner: "alice".into(),
            main: true,
        }),
        "新造的会话头一回写就记下"
    );
    let lane = new_id(now());
    miyu_session::fork(Fork {
        root: &home.root,
        owner: &alice_account(),
        parent: parent.id(),
        id: lane.clone(),
        cause: CommandId::parse("f1").expect("合写法"),
        by: alice(),
        at: now(),
        index: None,
        seal_after: None,
        within: None,
    })
    .await
    .expect("分叉得了");
    let branch = home.load_with(&lane, &talk, &tools).await;
    chat(&branch, "c2", "日语的敬语好难").await;
    stop(&branch).await;
    stop(&parent).await;
    home.memory.settle();
    let (index, _) = home.recall.turns(&room());
    let origins = index.origins().expect("读得了");
    assert_eq!(
        origins.get(parent.id().as_str()),
        Some(&Origin {
            owner: "alice".into(),
            main: true,
        }),
        "主线：属主、主会话"
    );
    assert_eq!(
        origins.get(lane.as_str()),
        Some(&Origin {
            owner: "alice".into(),
            main: false,
        }),
        "支线带 parent，不是主会话"
    );

    let search = Script::new([
        Play::calls(&[("memory_search", r#"{"query":"日语"}"#)]),
        Play::Says("找到了。"),
    ]);
    // 本机的主会话、有会话表：调用带列会话的端口，`history` 读得了别的会话。
    let lines = Lines {
        sessions: Some(Arc::new(Table::default()) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let asker = home
        .create_full(&search, &tools, Opening::default(), lines)
        .await;
    chat(&asker, "c3", "我以前说过日语的事吗").await;
    let result = last_result(&home.log(asker.id()));
    let date = now().local_date(miyu_kernel::time::UtcOffset::UTC);
    let short = parent.id().short().to_string();
    let lines: Vec<&str> = result.lines().collect();
    assert!(
        lines.contains(&format!("{date} session {short}: 我在学日语 / 好。").as_str()),
        "主线那一段带编号：{result}"
    );
    assert!(
        lines.contains(&format!("{date}: 日语的敬语好难 / 加油。").as_str()),
        "支线那一段不带编号、本机的不写在哪：{result}"
    );
    assert!(!result.contains(lane.short()), "支线的编号不出现：{result}");
    stop(&asker).await;
}

/// 照回合库判（不经会话）：属主一样的主会话打得开；别的账号的、场所会话（属主是桥的系统账号）、支线打不开；回合库没记过从哪
/// 来的（接上回合索引以前的库）照场所判，本机的当打得开、场所的打不开；没有问的人（协议搜的）一律不算打得开。
#[tokio::test]
async fn whether_she_can_open_it_follows_the_owner_and_main() {
    let home = Home::new();
    let (index, _) = home.recall.turns(&room());
    let sessions: Vec<_> = (0..6).map(|_| new_id(now())).collect();
    let put = |session: &miyu_kernel::id::SessionId, text: &str| {
        index
            .apply(
                session.as_str(),
                &[Edit::Put {
                    key: format!("{session}/1"),
                    text: text.into(),
                    at: now(),
                }],
                miyu_kernel::id::Seq::FIRST,
            )
            .expect("写得进");
    };
    let origin = |owner: &str, main: bool| Origin {
        owner: owner.into(),
        main,
    };
    let texts = [
        "日语 自己的主会话",
        "日语 别人的主会话",
        "日语 群里",
        "日语 支线",
        "日语 以前的本机会话",
        "日语 以前的群会话",
    ];
    for (session, text) in sessions.iter().zip(texts) {
        put(session, text);
    }
    let set =
        |n: usize, origin: Origin| index.origin(sessions[n].as_str(), &origin).expect("写得进");
    set(0, origin("alice", true));
    set(1, origin("bob", true));
    set(2, origin("im", true));
    set(3, origin("alice", false));
    index
        .place(sessions[2].as_str(), "qq:group:900001")
        .expect("写得进");
    index
        .place(sessions[5].as_str(), "qq:group:900001")
        .expect("写得进");
    let keeper = Keeper::new(&home.memory, room(), Listener::Owner(alice()));
    let opened = |asker: Option<&AccountId>| {
        let mut found: Vec<(String, bool, Option<VenueId>)> = keeper
            .turns("日语", 10, (None, asker), None)
            .expect("搜得了")
            .into_iter()
            .map(|turn| (turn.text, turn.opens, turn.venue))
            .collect();
        found.sort();
        found
    };
    let group = Some(VenueId::parse("qq:group:900001").expect("合写法"));
    let mut expected = vec![
        ("日语 自己的主会话".to_string(), true, None),
        ("日语 别人的主会话".to_string(), false, None),
        ("日语 群里".to_string(), false, group.clone()),
        ("日语 支线".to_string(), false, None),
        ("日语 以前的本机会话".to_string(), true, None),
        ("日语 以前的群会话".to_string(), false, group.clone()),
    ];
    expected.sort();
    assert_eq!(opened(Some(&alice_account())), expected);
    let mut nobody: Vec<(String, bool, Option<VenueId>)> = expected
        .into_iter()
        .map(|(text, _, venue)| (text, false, venue))
        .collect();
    nobody.sort();
    assert_eq!(opened(None), nobody, "没有问的人的一律打不开");
    let bob = AccountId::parse("bob").expect("合写法");
    assert!(
        opened(Some(&bob))
            .iter()
            .any(|(text, opens, _)| text == "日语 别人的主会话" && *opens),
        "bob 打得开他自己的"
    );
    assert!(
        !opened(Some(&bob))
            .iter()
            .any(|(text, opens, _)| text == "日语 自己的主会话" && *opens),
        "bob 打不开 alice 的"
    );
}

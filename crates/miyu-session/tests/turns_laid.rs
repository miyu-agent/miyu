//! 回合索引接上时边读边铺（施工 V-37 下，`docs/blueprint/memory.md` 第一条第 6 款）：真会话（记、搜记忆，撤销、恢复，中途换
//! 人格，分叉出来的支线接着说）的日志上，整份读出来照换人格那一条切开、`TurnFeed::primed_on`、`UseFeed::prime` 铺的，和照
//! `read_range` 一块块读、一条条喂 `Priming`、`UseFeed::see` 的，交回的 `TurnFeed`、`UseFeed`（`Debug` 写出来的每一样）、改动、
//! 用到的逐个相等，回合库照到哪落在各处。回合库里写进去的逐行一样，在 `memory/lane/tests/laid.rs` 照种子造的日志上比。

use std::path::Path;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{CommandId, Seq, SessionId};
use miyu_kernel::session::Command;
use miyu_recall::uses::{Use, UseFeed, since};
use miyu_recall::{Change, Priming, TurnFeed};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Fork, Handle, new_id};
use miyu_store::log::{Flow, read_range};
use miyu_store::recall::NO_PERSONA;
use miyu_tool::Catalog;

use crate::support::*;

/// 基础系统加记忆的三件，照包登记，和核心里一样。
fn catalog(home: &Home) -> Catalog {
    let resources = home.resources.path();
    Catalog::in_packages(vec![
        (
            miyu_tool::BASESYSTEM,
            miyu_basesystem::tools(resources).expect("读得出"),
        ),
        (
            miyu_memory::PACKAGE,
            miyu_memory::tools(resources).expect("读得出"),
        ),
    ])
    .expect("合写法")
}

/// 说一句，等这一轮结束。
async fn chat(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里最近一条换成人格 `persona`（无人格的是 [`NO_PERSONA`]）的那一条（同 `memory/recast.rs` 的 `floor`）。
fn floor(events: &[Event], persona: &str) -> Option<Seq> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::PolicyChanged(changed) => changed
            .persona
            .as_deref()
            .filter(|to| (if to.is_empty() { NO_PERSONA } else { to }) == persona)
            .map(|_| event.seq),
        _ => None,
    })
}

/// 铺好的：`TurnFeed`、`UseFeed` 写出来的样子，改动，用到的。
type Laid = (String, String, Vec<Change>, Vec<Use>);

/// 整份：底子、自己的都整份读出来，换人格以前的并进底子，`primed_on`、`prime`。
fn whole(base: &[Event], own: &[Event], persona: &str, after: Option<Seq>) -> Laid {
    let floor = floor(own, persona);
    let at = own.partition_point(|event| floor.is_some_and(|floor| event.seq <= floor));
    let mut joined = base.to_vec();
    joined.extend_from_slice(&own[..at]);
    let (feed, changes) = TurnFeed::primed_on(&joined, &own[at..], after);
    let mut uses = UseFeed::new("memory", miyu_tool::MEMORY_SEARCH);
    let used = uses.prime(&own[at..], after);
    (format!("{feed:?}"), format!("{uses:?}"), changes, used)
}

/// 一块块：底子照 `read_range` 读到分叉点，自己的从头读，一条条喂。
fn piece_by_piece(
    base: Option<(&Path, Seq)>,
    dir: &Path,
    floor: Option<Seq>,
    after: Option<Seq>,
) -> Laid {
    let mut priming = Priming::default();
    let mut uses = UseFeed::new("memory", miyu_tool::MEMORY_SEARCH);
    let mut used = Vec::new();
    if let Some((parent, upto)) = base {
        read_range(parent, Seq::FIRST, Some(upto), |events| {
            events.iter().for_each(|event| priming.base(event));
            Flow::Go
        })
        .expect("读得了");
    }
    read_range(dir, Seq::FIRST, None, |events| {
        for event in &events {
            if floor.is_some_and(|floor| event.seq <= floor) {
                priming.base(event);
            } else {
                priming.own(event);
                used.extend(uses.see(event));
            }
        }
        Flow::Go
    })
    .expect("读得了");
    let (feed, changes) = priming.finish(after);
    (
        format!("{feed:?}"),
        format!("{uses:?}"),
        changes,
        since(used, after),
    )
}

/// 一个会话（分叉出来的带底子）照每一间、照到哪落在各处，两样比。交回比了几处有改动的。
fn compare(home: &Home, session: &SessionId, base: Option<(&SessionId, Seq)>) -> usize {
    let own = home.log(session);
    let parent = base.map(|(parent, upto)| (home.root.session_dir(parent), upto));
    let base_events: Vec<Event> = match &parent {
        Some((dir, upto)) => miyu_store::log::read_events(dir)
            .expect("读得了")
            .into_iter()
            .filter(|event| event.seq <= *upto)
            .collect(),
        None => Vec::new(),
    };
    let dir = home.root.session_dir(session);
    let last = own.last().map_or(0, |event| event.seq.get());
    let mut changed = 0;
    for persona in ["engineer", "miyu", NO_PERSONA] {
        for after in [
            None,
            Some(1),
            Some(last / 3),
            Some(last / 2),
            Some(last - 1),
            Some(last),
        ] {
            let after = after.and_then(Seq::new);
            let want = whole(&base_events, &own, persona, after);
            let got = piece_by_piece(
                parent.as_ref().map(|(dir, upto)| (dir.as_path(), *upto)),
                &dir,
                floor(&own, persona),
                after,
            );
            assert_eq!(got, want, "{session} 记在 {persona} 那一间，照到 {after:?}");
            changed += usize::from(!want.2.is_empty());
        }
    }
    changed
}

#[tokio::test]
async fn real_sessions_lay_alike_whole_or_piece_by_piece() {
    let home = Home::new();
    let prompts = home.root.personas().join("miyu/prompts");
    std::fs::create_dir_all(&prompts).expect("建得了");
    std::fs::write(prompts.join("persona.md"), "You are Miyu.\n").expect("写得进");
    let script = Script::new([
        Play::calls(&[("remember", r#"{"class":"user","text":"团子爱吃鸡胸肉"}"#)]),
        Play::Says("记住了。"),
        Play::calls(&[("memory_search", r#"{"query":"团子"}"#)]),
        Play::Says("团子爱吃鸡胸肉。"),
        Play::Says("周末去爬山，带上水。"),
        Play::Says("<analysis>a</analysis>\n<summary>\n用户养猫、想去爬山。\n</summary>"),
        Play::Says("在。"),
        Play::Says("支线这边也在。"),
    ])
    .window(1_000_000);
    let tools = catalog(&home);
    let main = home.create_with(&script, &tools).await;
    chat(&main, "c1", "我养了一只猫叫团子").await;
    chat(&main, "c2", "团子爱吃什么").await;
    // 长一点：换人格先压一轮，有东西可压才开那一轮（同 `miyu-endpoint` 的 `persona_recast.rs`）。
    let long = format!("周末去爬山{}", "x".repeat(70_000));
    chat(&main, "c3", &long).await;
    assert!(
        ask(&main, "c4", Command::Revert { turn: None })
            .await
            .is_ok()
    );
    assert!(ask(&main, "c5", Command::Unrevert).await.is_ok());
    let recast = Command::Recast {
        persona: Some(Some("miyu".to_string())),
        preset: None,
    };
    let mut pushes = watch(&main).await;
    assert!(ask(&main, "c6", recast).await.is_ok());
    until_turn_ends(&mut pushes).await;
    chat(&main, "c7", "换了人格以后还记得吗").await;
    let lane = new_id(now());
    miyu_session::fork(Fork {
        root: &home.root,
        parent: main.id(),
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
    let branch = home.load_with(&lane, &script, &tools).await;
    chat(&branch, "c8", "支线里说一句").await;
    stop(&branch).await;
    stop(&main).await;
    home.memory.settle();

    let log = home.log(main.id());
    let kinds: Vec<&str> = log.iter().map(|event| event.body.kind()).collect();
    for kind in [
        "turn.reverted",
        "turn.unreverted",
        "session.policy_changed",
        "tool.result",
    ] {
        assert!(kinds.contains(&kind), "日志里有 {kind}：{kinds:?}");
    }
    let Some(Event {
        body: Body::SessionCreated(created),
        ..
    }) = home.log(&lane).into_iter().next()
    else {
        panic!("支线第一条是 session.created");
    };
    let forked = created.forked.expect("是分叉出来的");
    let changed =
        compare(&home, main.id(), None) + compare(&home, &lane, Some((main.id(), forked.upto)));
    assert!(changed > 10, "有改动的 {changed}");
}

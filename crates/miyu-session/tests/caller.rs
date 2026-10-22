//! 调用的事实（施工 X-43，`docs/blueprint/skills.md`「脚本」第 2 条）：执行器派工具时交场所和这一轮开口的是不是主人。本机的会话
//! 是主人；场所会话照这一轮的 `speakers`，陌生人不是、对应表认出的主人是；回合中途重启、载入以后接着干的那一轮不是（没有人开口）；
//! 脚本里调的照派它的那一轮。脚本开头写了 `Venues` 的只在那几种会话的工具面上。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::{Body, EndReason, Level, Permission};
use miyu_kernel::id::{ExternalId, VenueId};
use miyu_kernel::origin::{By, External};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Caller, Catalog, Tool};

use crate::support::*;

const GROUP: &str = "qq:group:1";

/// 群里的主人：对应表认出来的外部身份（带 `account`）。
fn owner_in_group() -> By {
    By::External(External {
        venue: VenueId::parse(GROUP).expect("合写法"),
        id: ExternalId::parse("qq:10001").expect("合写法"),
        account: Some(alice_account()),
        role: None,
    })
}

fn caller(venue: &str, owner: bool) -> Option<Caller> {
    Some(Caller {
        venue: VenueId::parse(venue).expect("合写法"),
        owner,
    })
}

fn catalog(tools: &[&Arc<Fake>]) -> Catalog {
    Catalog::new(tools.iter().map(|tool| Arc::clone(tool) as Arc<dyn Tool>)).expect("都合写法")
}

/// 场所 `venue` 里的会话：完全放开、没人能确认。
async fn session(home: &Home, script: &Script, tools: &Catalog, venue: &str) -> Handle {
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        attended: false,
        ..Opening::default()
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        ..Lines::default()
    };
    home.create_full(script, tools, opening, lines).await
}

/// `by` 说一句。
async fn say_as(handle: &Handle, by: By) {
    within("回应", handle.command(id("cmd-1"), by, say("hi")))
        .await
        .expect("会话在跑");
}

#[tokio::test]
async fn a_call_knows_where_it_is_and_whether_the_owner_spoke() {
    for (venue, by, owner) in [
        ("local", alice(), true),
        (GROUP, stranger(), false),
        (GROUP, owner_in_group(), true),
    ] {
        let home = Home::new();
        let note = Fake::new("note", Access::Read, Act::Echo);
        let script = Script::new([Play::calls(&[("note", "{}")]), Play::Says("好。")]);
        let handle = session(&home, &script, &catalog(&[&note]), venue).await;
        let mut pushes = watch(&handle).await;
        say_as(&handle, by).await;
        until_turn_ends(&mut pushes).await;
        let calls = note.calls();
        assert_eq!(calls.len(), 1, "{venue}：{:?}", home.log(handle.id()));
        assert_eq!(calls[0].caller, caller(venue, owner), "{venue}");
    }
}

#[tokio::test]
async fn a_call_made_from_a_script_carries_the_round_that_sent_it() {
    for (by, owner) in [(owner_in_group(), true), (stranger(), false)] {
        let home = Home::new();
        let shell = Fake::new("shell", Access::Read, Act::Calls(vec![("note", "{}")]));
        let note = Fake::callable("note", Access::Read, Act::Echo);
        let script = Script::new([
            Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
            Play::Says("好。"),
        ]);
        let handle = session(&home, &script, &catalog(&[&shell, &note]), GROUP).await;
        let mut pushes = watch(&handle).await;
        say_as(&handle, by).await;
        until_turn_ends(&mut pushes).await;
        let calls = note.calls();
        assert_eq!(calls.len(), 1, "脚本里调到了：{:?}", home.log(handle.id()));
        assert_eq!(calls[0].caller, caller(GROUP, owner), "照派它的那一轮");
        assert_eq!(shell.calls()[0].caller, caller(GROUP, owner));
    }
}

#[tokio::test]
async fn the_round_resumed_after_a_restart_is_not_the_owners() {
    let home = Home::new();
    let note = Fake::new("note", Access::Read, Act::Echo);
    let tools = catalog(&[&note]);
    let before = Script::new([Play::Holds]);
    let handle = session(&home, &before, &tools, GROUP).await;
    let session_id = handle.id().clone();
    let mut pushes = watch(&handle).await;
    say_as(&handle, owner_in_group()).await;
    until_delta(&mut pushes).await;
    stop(&handle).await;

    // 载入：被重启打断的那一轮接着干，开它的是那一条结束、不是人话，算不出开口的人。
    let after = Script::new([Play::calls(&[("note", "{}")]), Play::Says("好。")]);
    let _loaded = home.load_with(&session_id, &after, &tools).await;
    until_logged(&home, &session_id, |log| {
        log.iter().any(|event| {
            matches!(&event.body, Body::TurnEnded(ended) if ended.reason == EndReason::Completed)
        })
    })
    .await;
    let calls = note.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].caller, caller(GROUP, false), "拿不到的照最严");
}

/// 照开头 `text` 写一个脚本 `file`，当工具。
fn script_tool(home: &Home, file: &str, text: &str) -> Arc<dyn Tool> {
    let path = home.root.path().join(file);
    std::fs::write(&path, text).expect("写得进");
    let head = miyu_policy::script::read(file, text).expect("开头写对了");
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    miyu_basesystem::script(&resources, &head, path, "~home").expect("资源读得出来")
}

#[tokio::test]
async fn a_script_is_offered_only_where_its_head_says() {
    for (venue, by, near) in [("local", alice(), true), (GROUP, stranger(), false)] {
        let home = Home::new();
        let tools = Catalog::new([
            script_tool(
                &home,
                "near.py",
                "# Description: Near.\n# Venues: local\n# Need load: no\n",
            ),
            script_tool(
                &home,
                "any.py",
                "# Description: Anywhere.\n# Need load: no\n",
            ),
        ])
        .expect("合写法");
        let script = Script::new([Play::Says("好。")]);
        let handle = session(&home, &script, &tools, venue).await;
        let mut pushes = watch(&handle).await;
        say_as(&handle, by).await;
        until_turn_ends(&mut pushes).await;
        let requests = script.requests();
        let names: Vec<&str> = requests[0]
            .1
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect();
        assert_eq!(names.contains(&"near"), near, "{venue}：{names:?}");
        assert!(names.contains(&"any"), "{venue}：没写的两边都有：{names:?}");
    }
}

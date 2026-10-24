//! show 的那一句（施工 D-5 补二，`docs/blueprint/tools/show.md`）：工具面上有 `show`、开关开着的会话 system 里带；从
//! `tools.rs` 拆出来守 500 行上限。

use std::sync::Arc;

use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 这几件假工具的目录。
fn catalog(tools: &[&Arc<Fake>]) -> Catalog {
    Catalog::new(tools.iter().map(|tool| Arc::clone(tool) as Arc<dyn Tool>)).expect("都合写法")
}

/// 说一句，等这一轮说完。
async fn turn(handle: &Handle, command: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// show 的那一句（施工 D-5 补二）：工具面上有 `show` 的会话 system 里带，接在最后单成一段；没有的不带。
#[tokio::test]
async fn only_a_session_with_show_carries_the_show_line() {
    let line = include_str!("../../../resources/software/basesystem/show/system.txt").trim_end();
    let home = Home::new();
    let show = Fake::new("show", Access::Read, Act::Echo);
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create_with(&script, &catalog(&[&show])).await;
    turn(&handle, "cmd-1").await;
    let with = script.requests()[0].1.clone();
    assert!(
        with.system.contains(&format!("\n\n{line}")),
        "{}",
        with.system
    );
    let read = Fake::new("read", Access::Read, Act::Echo);
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create_with(&script, &catalog(&[&read])).await;
    turn(&handle, "cmd-1").await;
    let without = script.requests()[0].1.clone();
    assert!(!without.system.contains(line), "{}", without.system);
}

/// show 那一句的开关（施工 D-5 补二，项目主人 10-24 定：`basesystem.show.system_hint`，默认开）：关了的会话工具面上有
/// `show` 也不带；开着的照带。
#[tokio::test]
async fn the_show_line_follows_its_switch() {
    let line = include_str!("../../../resources/software/basesystem/show/system.txt").trim_end();
    let show = Fake::new("show", Access::Read, Act::Echo);
    for (source, carried) in [
        ("[basesystem.show]\nsystem_hint = false\n", false),
        ("[basesystem.show]\nsystem_hint = true\n", true),
    ] {
        let mut home = Home::new();
        home.configs = crate::support::routing::configs(source, &[]);
        let script = Script::new([Play::Says("好。")]);
        let handle = home.create_with(&script, &catalog(&[&show])).await;
        turn(&handle, "cmd-1").await;
        let system = script.requests()[0].1.system.clone();
        assert_eq!(system.contains(line), carried, "{source}{system}");
    }
}

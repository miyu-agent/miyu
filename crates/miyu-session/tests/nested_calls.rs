//! 脚本调工具的执行器这一头（施工 M-2，设计 `39-脚本调工具.md` 第四节）：直接调的 `shell` 拿到端口，经它调的每一件照
//! 直接调用的链判、在自己的任务里跑，记一条 `tool.nested`、落了盘才答回去；`clicall` 没开的、不认识的当场拒、不记；跑完的
//! 结果存成 blob（有 `data` 的是它的一行 JSON），交给脚本的有字、`data`，图不交；报了结束回合的丢掉。父调用被打断，在跑的
//! 掐掉、记「已取消」，答回去的也是「已取消」。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, NestedStatus, ToolNested, ToolResult};
use miyu_kernel::session::{Command, Queued};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

fn catalog(tools: &[&Arc<Fake>]) -> Catalog {
    Catalog::new(tools.iter().map(|tool| Arc::clone(tool) as Arc<dyn Tool>)).expect("都合写法")
}

fn nested(log: &[Event]) -> Vec<&ToolNested> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolNested(nested) => Some(nested),
            _ => None,
        })
        .collect()
}

fn shell_result(log: &[Event]) -> (usize, &ToolResult) {
    log.iter()
        .enumerate()
        .find_map(|(at, event)| match &event.body {
            Body::ToolResult(result) => Some((at, result)),
            _ => None,
        })
        .expect("shell 有结果")
}

fn text(result: &ToolResult) -> String {
    result
        .blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.as_str(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

#[tokio::test]
async fn a_script_calls_tools_through_the_kernel_and_hears_back_after_the_log() {
    let shell = Fake::new(
        "shell",
        Access::Read,
        Act::Calls(vec![
            ("web_search", r#"{"query":"niri"}"#),
            ("lookup", "{}"),
            ("nope", "{}"),
            ("broken", "{}"),
            ("picture", "{}"),
            ("ender", "{}"),
        ]),
    );
    let search = Fake::callable(
        "web_search",
        Access::Read,
        Act::Gives("found 1", r#"{"kind":"links","links":[{"title":"niri"}]}"#),
    );
    let lookup = Fake::new("lookup", Access::Read, Act::Echo);
    let broken = Fake::callable("broken", Access::Read, Act::Fails("no luck"));
    let picture = Fake::callable("picture", Access::Read, Act::Shows(&[1, 2, 3]));
    let ender = Fake::callable("ender", Access::Read, Act::Ends);
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
        Play::Says("好了。"),
    ]);
    let handle = home
        .create_with(
            &script,
            &catalog(&[&shell, &search, &lookup, &broken, &picture, &ender]),
        )
        .await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("数一下"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;

    let log = home.log(handle.id());
    let (at, result) = shell_result(&log);
    assert_eq!(
        text(result),
        [
            r#"web_search ok text=found 1 data={"kind":"links","links":[{"title":"niri"}]}"#,
            "lookup refused NotAllowed",
            "nope refused Unknown",
            "broken error text=no luck",
            "picture ok text=shown images dropped",
            "ender ok text=ended",
        ]
        .join("\n"),
        "脚本照落了盘的答拿到结果"
    );
    let settled = nested(&log);
    let ids: Vec<String> = settled
        .iter()
        .map(|n| format!("{} {} {}", n.id, n.name, n.status.as_str()))
        .collect();
    let call = result.call_id.to_string();
    assert_eq!(
        ids,
        [
            format!("{call}#1 web_search ok"),
            format!("{call}#4 broken error"),
            format!("{call}#5 picture ok"),
            format!("{call}#6 ender ok"),
        ],
        "拒了的不记；编号照脚本调的先后"
    );
    assert!(
        log[..at]
            .iter()
            .filter(|event| matches!(event.body, Body::ToolNested(_)))
            .count()
            == 4,
        "都记在 shell 的结果前面"
    );
    let blobs = Blobs::new(home.root.blobs(&alice_account()));
    let first = settled[0].result.as_ref().expect("存了结果");
    let line = br#"{"kind":"links","links":[{"title":"niri"}]}"#;
    assert_eq!(
        blobs.get(first).expect("读得出"),
        line,
        "有 data 的存它的一行 JSON"
    );
    assert_eq!(settled[0].bytes, Some(line.len() as u64));
    assert_eq!(
        blobs.get(settled[1].result.as_ref().unwrap()).unwrap(),
        b"no luck"
    );
    assert!(
        !log.iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_)) && event.seq < log[at].seq),
        "嵌套调用里报的结束回合不算"
    );
    assert_eq!(script.requests().len(), 2, "shell 的结果回来以后照常请求");
}

#[tokio::test]
async fn interrupting_the_shell_cancels_what_runs_under_it() {
    let shell = Fake::new("shell", Access::Read, Act::Calls(vec![("slow", "{}")]));
    let slow = Fake::callable("slow", Access::Read, Act::Holds);
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
        Play::Says("好。"),
    ]);
    let handle = home.create_with(&script, &catalog(&[&shell, &slow])).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("跑")).await.expect("会话在跑");
    within("嵌套调用跑起来", async {
        while slow.calls().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    within("在跑的被掐掉", async {
        while slow.dropped() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(shell.calls()[0].nested.is_some(), "直接调的 shell 拿到端口");
    let under = &slow.calls()[0];
    assert!(
        under.jobs.is_none()
            && under.agents.is_none()
            && under.questions.is_none()
            && under.goals.is_none()
            && under.nested.is_none(),
        "嵌套调用里跑的不能起任务、派子代理、问人、改目标，也不再开端口"
    );
    let log = home.log(handle.id());
    let settled = nested(&log);
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].status, NestedStatus::Cancelled);
    let (at, _) = shell_result(&log);
    assert!(
        log[..at]
            .iter()
            .any(|event| matches!(event.body, Body::ToolNested(_))),
        "取消的记在 shell 的结果前面"
    );
}

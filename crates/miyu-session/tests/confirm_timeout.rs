//! 等人确认到时自动拒（施工 CD-1，`docs/blueprint/kernel/asking.md`「到时没人确认」，配置 `permission.confirm.timeout`）：会话
//! actor 照这一轮的配置把等多久交给内核，内核记请求时写上截止的时刻、交出到时叫醒，actor 起定时的任务，到时送回去。没人确认
//! 的，内核替人拒绝，她拿到那一句接着干；人先答了的照常跑；配 `0` 的一直等；没配的等 5 分钟。
//!
//! 一律用一件改装了什么的假工具（访问类别 `install`）：完全放开下每次都问人（`guard_install.rs`）。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::event::{Body, Decision, Event, Level, Permission, ToolStatus};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Answer, Command};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 到时写给她的那一句：出厂的原文。
const WORDS: &str = "Not run: no one answered the approval request in time.";

/// 在工作区里造一个完全放开、有人能确认的会话，配置是 `config`（没有的照出厂），调一次 `package`，等请求落了盘。交回假工具、
/// 会话、剧本。
async fn asked(home: &mut Home, config: Option<&str>) -> (Arc<Fake>, Handle, Script) {
    if let Some(text) = config {
        home.configs = crate::support::routing::configs(text, &[]);
    }
    let tool = Fake::new("package", Access::Install, Act::Echo);
    let catalog = Catalog::new([Arc::clone(&tool) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([
        Play::calls(&[("package", r#"{"remove":"kit"}"#)]),
        Play::Says("那我换个办法。"),
    ]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        attended: true,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
    };
    let handle = home.create_as(&script, &catalog, opening).await;
    ask(&handle, "cmd-1", say("卸了 kit"))
        .await
        .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        kinds(log).contains(&"tool.approval_requested")
    })
    .await;
    (tool, handle, script)
}

/// 日志里的确认请求：记下的时刻到截止的时刻差几毫秒，一直等的没有。
fn deadline_ms(log: &[Event]) -> Option<i64> {
    log.iter().find_map(|event| match &event.body {
        Body::ApprovalRequested(request) => Some(
            request
                .expires_at
                .map(|at| at.unix_millis() - event.at.unix_millis()),
        ),
        _ => None,
    })?
}

/// 日志里的决定：选项、谁定的。
fn decisions(log: &[Event]) -> Vec<(Decision, By)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ApprovalDecided(decided) => Some((decided.decision.clone(), event.by.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn left_unanswered_it_is_denied_after_the_timeout_and_she_goes_on() {
    let mut home = Home::outside_temp();
    let (tool, handle, script) =
        asked(&mut home, Some("[permission.confirm]\ntimeout = 1\n")).await;
    assert_eq!(deadline_ms(&home.log(handle.id())), Some(1000));
    let log = until_logged(&home, handle.id(), |log| kinds(log).contains(&"turn.ended")).await;
    assert_eq!(
        decisions(&log),
        [(Decision::Deny, By::Kernel)],
        "内核替人拒的"
    );
    let (result, by) = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some((result.clone(), event.by.clone())),
            _ => None,
        })
        .expect("有结果");
    assert_eq!((result.status, by), (ToolStatus::Denied, By::Kernel));
    assert_eq!(
        result.human.as_ref().map(|said| said.key.as_str()),
        Some("core/tool-results/approval-timed-out")
    );
    assert!(tool.calls().is_empty(), "没跑");
    // 她看到的是那一句，接着说了话。
    let requests = script.requests();
    assert_eq!(requests.len(), 2, "拿到结果又问了一次");
    assert!(
        format!("{:?}", requests[1].1).contains(WORDS),
        "她看得到那一句"
    );
}

#[tokio::test]
async fn an_answer_in_time_runs_it_and_the_deadline_passes_quietly() {
    let mut home = Home::outside_temp();
    let (tool, handle, _script) =
        asked(&mut home, Some("[permission.confirm]\ntimeout = 1\n")).await;
    let call_id = home
        .log(handle.id())
        .iter()
        .find_map(|event| match &event.body {
            Body::ApprovalRequested(request) => Some(request.call_id),
            _ => None,
        })
        .expect("有请求");
    let answer = Answer::Approval {
        decision: Decision::Once,
        reason: None,
        attachments: Vec::new(),
        nested: None,
    };
    ask(&handle, "cmd-2", Command::Answer { call_id, answer })
        .await
        .expect("会话在跑");
    until_logged(&home, handle.id(), |log| kinds(log).contains(&"turn.ended")).await;
    // 过了截止的时刻再看：到时送回去了也不理。
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let log = home.log(handle.id());
    assert!(
        matches!(
            decisions(&log).as_slice(),
            [(Decision::Once, By::Person(_))]
        ),
        "{:?}",
        decisions(&log)
    );
    assert_eq!(tool.calls().len(), 1, "照常跑了");
    assert_eq!(
        kinds(&log)
            .iter()
            .filter(|kind| **kind == "tool.result")
            .count(),
        1
    );
}

#[tokio::test]
async fn zero_waits_forever_and_unset_waits_five_minutes() {
    let mut home = Home::outside_temp();
    let (tool, handle, _script) =
        asked(&mut home, Some("[permission.confirm]\ntimeout = 0\n")).await;
    assert_eq!(deadline_ms(&home.log(handle.id())), None, "0 是一直等");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let log = home.log(handle.id());
    assert!(decisions(&log).is_empty(), "还在等：{:?}", kinds(&log));
    assert!(!kinds(&log).contains(&"tool.result"));
    assert!(tool.calls().is_empty());

    let mut home = Home::outside_temp();
    let (_, handle, _) = asked(&mut home, None).await;
    assert_eq!(
        deadline_ms(&home.log(handle.id())),
        Some(300_000),
        "出厂 5 分钟"
    );
}

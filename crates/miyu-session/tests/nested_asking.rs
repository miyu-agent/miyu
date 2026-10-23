//! 脚本里调的要人确认（施工 M-7 上，`docs/blueprint/kernel/asking.md`「脚本里调的要人确认」）：执行器这一头。前台、有人能
//! 确认的会话里，链说要问人的停下来问，请求挂在那次 `shell` 上；`shell` 收到「在等」，答了收到「等完了」；不带序号的回答
//! 答得上（现在的头就这么答）；拒绝的脚本拿到直接调用的那一句。没人能确认的照旧 `needs_approval`。

use std::sync::Arc;

use miyu_kernel::event::{ApprovalRequested, Body, Decision, Event, ToolResult};
use miyu_kernel::session::{Answer, Command};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 一个脚本：经端口调一次要联网的 `fetch`。工作区这一级联网要问人。
async fn scripted(home: &Home, attended: bool) -> miyu_session::Handle {
    let shell = Fake::new("shell", Access::Read, Act::Calls(vec![("fetch", "{}")]));
    let fetch = Fake::callable("fetch", Access::Network, Act::Echo);
    let script = Script::new([
        Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
        Play::Says("好了。"),
    ]);
    let catalog = Catalog::new([shell, fetch].map(|tool| tool as Arc<dyn Tool>)).expect("都合写法");
    let opening = Opening {
        attended,
        ..Opening::default()
    };
    home.create_as(&script, &catalog, opening).await
}

fn requests(log: &[Event]) -> Vec<ApprovalRequested> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ApprovalRequested(request) => Some(request.clone()),
            _ => None,
        })
        .collect()
}

fn shell_text(log: &[Event]) -> String {
    let result: &ToolResult = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("shell 有结果");
    result
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.as_str(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 不带序号回答（现在的头就这么答）。
async fn answer(handle: &miyu_session::Handle, request: &ApprovalRequested, decision: Decision) {
    let reason = (decision == Decision::Deny).then(|| "别联网".to_string());
    let answer = Answer::Approval {
        decision,
        reason,
        attachments: Vec::new(),
        nested: None,
    };
    let call_id = request.call_id;
    ask(handle, "cmd-2", Command::Answer { call_id, answer })
        .await
        .expect("会话在跑");
}

#[tokio::test]
async fn a_script_waits_for_the_person_and_hears_back_once_allowed() {
    let home = Home::new();
    let handle = scripted(&home, true).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("抓一下"))
        .await
        .expect("会话在跑");
    let session = handle.id().clone();
    let log = until_logged(&home, &session, |log| !requests(log).is_empty()).await;
    let request = requests(&log).remove(0);
    assert_eq!(request.nested, Some(1), "挂在 shell 上、带序号");
    assert_eq!(request.access, Access::Network);
    answer(&handle, &request, Decision::Once).await;
    until_turn_ends(&mut pushes).await;
    let log = home.log(&session);
    assert_eq!(
        shell_text(&log),
        "fetch ok text=cwd=~/src/miyu args={}\nheld true,false",
        "答了才跑；shell 先收到在等、再收到等完了"
    );
}

#[tokio::test]
async fn a_denial_reaches_the_script_with_the_words_of_a_direct_call() {
    let home = Home::new();
    let handle = scripted(&home, true).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("抓一下"))
        .await
        .expect("会话在跑");
    let session = handle.id().clone();
    let log = until_logged(&home, &session, |log| !requests(log).is_empty()).await;
    answer(&handle, &requests(&log)[0], Decision::Deny).await;
    until_turn_ends(&mut pushes).await;
    let text = shell_text(&home.log(&session));
    assert!(text.starts_with("fetch denied reason="), "{text}");
    assert!(text.contains("别联网"), "带上理由：{text}");
    assert!(text.ends_with("held true,false"), "{text}");
}

#[tokio::test]
async fn without_anyone_to_ask_it_is_settled_on_the_spot() {
    let home = Home::new();
    let handle = scripted(&home, false).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("抓一下"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(handle.id());
    assert!(requests(&log).is_empty(), "不问");
    assert_eq!(shell_text(&log), "fetch needs_approval");
}

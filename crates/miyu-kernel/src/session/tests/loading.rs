//! 要先加载的工具（施工 L-1，`docs/designs/25-工具加载.md` W9）：`load_tools` 由内核当场答，`by` 是内核，成了的是 ok、不派给
//! 执行器；参数不对的当场出错，写成字符串的列表照参数格式修正；要先加载的工具没加载直接调、出错了的，结果后面附上完整格式，
//! 成了的、不用加载的不附；会话没有要先加载的，`load_tools` 照不认识的工具答。

use std::collections::BTreeMap;

use super::executor::*;
use super::*;
use crate::event::{Said, ToolStatus};
use crate::id::CallId;
use crate::tool::{Contract, Loading, LoadingSources};

/// 工具面上 `read` 要先加载，多一件 `load_tools`。
fn loading_policy() -> Policy {
    let mut policy = policy();
    policy.tools.insert(
        "load_tools".to_string(),
        ToolRule {
            access: Access::Read,
            parameters: serde_json::from_str(
                r#"{"type":"object","properties":{"tools":{"type":"array","items":{"type":"string"}}}}"#,
            )
            .unwrap(),
            clicall: false,
            command_only: false,
        },
    );
    let contracts = BTreeMap::from([(
        "read".to_string(),
        Contract {
            description: "Read a file.".to_string(),
            parameters: serde_json::from_str(
                r#"{"type":"object","properties":{"path":{"type":"string"}}}"#,
            )
            .unwrap(),
        },
    )]);
    let sources = LoadingSources {
        unknown: "Nothing to load for {names}.\n",
        bad_args: "Name the tools.\n",
        contract: "Full parameters of {name}:\n",
    };
    policy.loading = Some(Loading::new("load_tools".to_string(), contracts, sources).unwrap());
    policy
}

/// 同 `asking`，策略是 `policy`。
fn asking_with(policy: Policy) -> Session {
    let mut session = session_with(policy);
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    session
}

/// 这一条结果里的全部字，接在一起。
fn text_of(event: &Event) -> String {
    let Body::ToolResult(result) = &event.body else {
        panic!("应该是 tool.result：{event:?}");
    };
    result
        .blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

const READ: &str = r#"{"name":"read","description":"Read a file.","parameters":{"type":"object","properties":{"path":{"type":"string"}}}}"#;

#[test]
fn load_tools_is_answered_on_the_spot() {
    let mut session = asking_with(loading_policy());
    let actions = call_tools(
        &mut session,
        5,
        &[("load_tools", r#"{"tools":["read","grep"]}"#)],
    );
    let events = appended_events(&actions);
    assert_eq!(
        result_of(&events[2]),
        (
            call(6, 1),
            ToolStatus::Ok,
            By::Kernel,
            format!("{READ}\nNothing to load for grep.\n")
        )
    );
    assert_eq!(
        said_of(&events[2]),
        Some(Said::new("core/loading/loaded").with("tools", "read"))
    );
    assert!(
        ran(&allowing(&mut session, stored(appended(&actions)[2].get()))).is_empty(),
        "不派给执行器"
    );
}

#[test]
fn a_list_written_as_a_string_is_repaired_and_no_list_is_an_error() {
    let mut session = asking_with(loading_policy());
    let actions = call_tools(
        &mut session,
        5,
        &[("load_tools", r#"{"tools":"[\"read\"]"}"#)],
    );
    assert_eq!(
        result_of(&appended_events(&actions)[2]).3,
        format!("{READ}\n")
    );
    let mut session = asking_with(loading_policy());
    let actions = call_tools(&mut session, 5, &[("load_tools", r#"{"names":["read"]}"#)]);
    let events = appended_events(&actions);
    assert_eq!(
        result_of(&events[2]),
        (
            call(6, 1),
            ToolStatus::Error,
            By::Kernel,
            "Name the tools.\n".to_string()
        )
    );
    assert_eq!(
        said_of(&events[2]),
        Some(Said::new("core/loading/bad-args"))
    );
}

/// 执行完了，出错，说了 `text`。
fn failed_done(call_id: CallId, text: &str) -> Input {
    let Input::ToolDone {
        at,
        blocks,
        duration_ms,
        human,
        effects,
        stopped,
        end_turn,
        display_name,
        ..
    } = done(call_id, text)
    else {
        unreachable!("done 交回的是 ToolDone");
    };
    Input::ToolDone {
        at,
        call_id,
        error: true,
        blocks,
        duration_ms,
        human,
        effects,
        stopped,
        end_turn,
        sent: Vec::new(),
        display_name,
        detail: None,
    }
}

#[test]
fn an_error_of_a_tool_to_load_comes_with_its_contract() {
    let mut session = asking_with(loading_policy());
    call_tools(&mut session, 5, &[("read", r#"{"file":"a"}"#)]);
    allowing(&mut session, stored(7));
    let actions = allowing(&mut session, failed_done(call(6, 1), "no path given"));
    let events = appended_events(&actions);
    assert_eq!(
        text_of(&events[0]),
        r#"no path givenFull parameters of read:
{"type":"object","properties":{"path":{"type":"string"}}}
"#
    );
    // 成了的不附。
    let mut session = asking_with(loading_policy());
    call_tools(&mut session, 5, &[("read", r#"{"path":"a"}"#)]);
    allowing(&mut session, stored(7));
    let actions = allowing(&mut session, done(call(6, 1), "fine"));
    assert_eq!(text_of(&appended_events(&actions)[0]), "fine");
}

#[test]
fn without_tools_to_load_load_tools_is_unknown() {
    let mut session = asking();
    let actions = call_tools(&mut session, 5, &[("load_tools", r#"{"tools":["read"]}"#)]);
    assert_eq!(
        result_of(&appended_events(&actions)[2]).3,
        "no tool load_tools".to_string()
    );
    // 出错了也不附：会话没有要先加载的。
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", r#"{"file":"a"}"#)]);
    allowing(&mut session, stored(7));
    let actions = allowing(&mut session, failed_done(call(6, 1), "no path given"));
    assert_eq!(text_of(&appended_events(&actions)[0]), "no path given");
}

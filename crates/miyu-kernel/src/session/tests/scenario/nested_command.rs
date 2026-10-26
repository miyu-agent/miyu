//! 场景：不上工具面的工具（施工 BS-1 中，设计 `44-基础系统拆包.md` 第三节第 4 条）。她直接调的当不认识，脚本里的
//! `miyu call` 也当不认识；只有会话里的 `miyu pkg`（核心自带的命令）经通道调得到，访问类别那道硬规矩、`clicall` 都不查，
//! 照常过链；`miyu pkg` 也只调得了它们。

use super::nested::{SHELL, answered, running_shell, settled};
use super::*;
use crate::event::{Said, ToolStatus};
use crate::tool::{Access, ToolRule};

/// 替身的策略多一件只给核心自带的命令调的 `package`（访问类别 `install`）。
fn with_package() -> Policy {
    let mut made = policy();
    made.tools.insert(
        "package".to_string(),
        ToolRule {
            access: Access::Install,
            parameters: serde_json::from_str(r#"{"type":"object"}"#).unwrap(),
            clicall: true,
            command_only: true,
        },
    );
    made
}

#[test]
fn only_the_command_reaches_a_tool_off_the_face() {
    let mut stage = Stage::new(with_package, environment("~/src/miyu"), at(0));
    running_shell(&mut stage);
    stage.nested_call(&format!("{SHELL}#1"), "package", "{}");
    stage.nested_command(&format!("{SHELL}#2"), "read", "{}");
    stage.nested_command(&format!("{SHELL}#3"), "package", r#"{"install":"a"}"#);
    assert_eq!(
        answered(&stage),
        [
            "call_6_1#1 refused Unknown",
            "call_6_1#2 refused NotAllowed",
        ]
    );
    // 访问类别是 `install` 也收下、过链（链放行了就跑），不当场拒。
    let ran: Vec<String> = stage
        .nested_ran()
        .iter()
        .map(|(id, name, _)| format!("{id} {name}"))
        .collect();
    assert_eq!(ran, ["call_6_1#3 package"]);
    stage.nested_done("call_6_1#3", false);
    assert_eq!(settled(&stage), ["call_6_1#3 ok"]);
}

#[test]
fn she_cannot_call_a_tool_off_the_face() {
    let mut stage = Stage::new(with_package, environment("~/src/miyu"), at(0));
    stage.model([
        Line::calls("装一个。", &[("package", r#"{"install":"a"}"#)]),
        Line::says("装不了。"),
    ]);
    stage.say("装个包");
    let result = stage
        .log()
        .iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.clone()),
            _ => None,
        })
        .expect("当场答了一句");
    assert_eq!(result.status, ToolStatus::Error);
    assert_eq!(
        result.human,
        Some(Said::new("core/tool-results/unknown").with("name", "package"))
    );
    assert!(stage.ran().is_empty(), "没交给执行器");
}

#[test]
fn the_executor_opens_the_channel_for_tools_off_the_face() {
    // 读的那件（同出厂的 `packages`）：类别准、`clicall` 写着开也不列给脚本，`miyu call` 看不见它。
    let mut made = with_package();
    made.tools.insert(
        "packages".to_string(),
        ToolRule {
            access: Access::Read,
            parameters: serde_json::from_str(r#"{"type":"object"}"#).unwrap(),
            clicall: true,
            command_only: true,
        },
    );
    let session = session_with(made);
    assert!(session.has_command_tools());
    let listed: Vec<String> = session
        .clicall_tools()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(!listed.contains(&"package".to_string()));
    assert!(!listed.contains(&"packages".to_string()));
    assert!(!session_with(policy()).has_command_tools());
}

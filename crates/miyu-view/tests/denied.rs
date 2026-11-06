//! 被拒的那一步，标题配上结果给人看的那一句（施工 HK-1补）：守卫拒的写入、权限策略拒的命令、只读时内核拒的写入，不分
//! 工具都写 `title.said`；被拒的结果没带给人看的说法的，照旧不写。

use std::sync::Arc;

use miyu_kernel::event::Said;
use miyu_kernel::id::ModuleId;
use miyu_kernel::session::Verdict;
use miyu_kernel::testkit::{Line, Play, Stage};
use miyu_view::{Entry, Projector};

use crate::support::*;

/// 只喂落了盘的，照连接是 `language` 的字（同 [`page`]，换了语言）。
fn paged_in(stage: &Stage, language: &str) -> Vec<Entry> {
    let mut projector = Projector::new(Arc::new(texts(language)), None);
    for event in stage.log() {
        projector.event(event);
    }
    projector.entries().to_vec()
}

/// 模型调一次 `name`，参数是 `args`，执行前的链照 `verdict` 判；说完以后回一句。
fn denied(name: &str, args: &str, verdict: Verdict) -> Stage {
    let mut stage = stage();
    stage.model([Line::calls("", &[(name, args)]), Line::says("ok")]);
    stage.guards([verdict]);
    stage.tools([Play::done("ran")]);
    stage.say("go");
    stage
}

/// 唯一那一步工具的标题。
fn title(entries: &[Entry]) -> serde_json::Value {
    let tools = of_kind(entries, "tool");
    assert_eq!(tools.len(), 1, "{entries:#?}");
    let tool = json(tools[0]);
    assert_eq!(tool["state"], "denied", "{tool}");
    tool["title"].clone()
}

/// 守卫拒绝、写了理由：模型看到的是英文的那一句，人看到的照 `core/guard/denied-with-reason`。
fn guarded() -> Verdict {
    Verdict::Deny {
        module: ModuleId::parse("acme-guard").expect("合模块编号的写法"),
        text: "Blocked by acme-guard: no writes on Friday".to_string(),
        human: Some(
            Said::new("core/guard/denied-with-reason")
                .with("reason", "周五不许写")
                .with("package", "acme-guard"),
        ),
    }
}

#[test]
fn a_write_blocked_by_a_guard_says_why_in_the_title() {
    let stage = denied(
        "write",
        r#"{"file_path":"/home/alice/a.txt","content":"x"}"#,
        guarded(),
    );
    let zh = title(&paged_in(&stage, "zh"));
    assert_eq!(zh["said"], "acme-guard 拦下了：周五不许写", "{zh}");
    let en = title(&same(&stage));
    assert_eq!(en["said"], "Blocked by acme-guard: 周五不许写", "{en}");
    assert_eq!(en["object"], "~/a.txt", "对象照旧：{en}");
}

#[test]
fn an_edit_blocked_by_a_guard_says_why_too() {
    let stage = denied(
        "edit",
        r#"{"file_path":"/home/alice/a.txt","old_string":"a","new_string":"b"}"#,
        guarded(),
    );
    let zh = title(&paged_in(&stage, "zh"));
    assert_eq!(zh["said"], "acme-guard 拦下了：周五不许写", "{zh}");
}

#[test]
fn a_command_refused_by_the_permission_policy_says_its_own_words() {
    // 权限策略拒的（`miyu-session` 的 `guard.rs` 的 `deny`）：模块是 `permissions`，说法是 `core/permissions/…`。
    let stage = denied(
        "shell",
        r#"{"command":"rm -rf build","description":"Clean"}"#,
        Verdict::Deny {
            module: ModuleId::parse("permissions").expect("合模块编号的写法"),
            text: "You cannot run commands here.".to_string(),
            human: Some(Said::new("core/permissions/no-commands")),
        },
    );
    let zh = title(&paged_in(&stage, "zh"));
    assert_eq!(zh["said"], "这里不能执行命令", "{zh}");
    assert_eq!(zh["object"], "Clean", "命令的对象照旧是短标题：{zh}");
    let en = title(&same(&stage));
    assert_eq!(en["said"], "Commands are not available here", "{en}");
}

#[test]
fn a_write_refused_while_read_only_says_so() {
    // 只读时内核当场拦下（`miyu-kernel` 的 `session/tools.rs`），说法是 `core/tool-results/read-only`。
    let mut stage = stage();
    stage.set_permission(None, Some(true));
    stage.model([
        Line::calls("", &[("write", r#"{"file_path":"a","content":"x"}"#)]),
        Line::says("ok"),
    ]);
    stage.say("go");
    let zh = title(&paged_in(&stage, "zh"));
    assert_eq!(zh["said"], "只读模式，未执行", "{zh}");
    same(&stage);
}

#[test]
fn a_denial_without_words_for_people_stays_without() {
    let stage = denied(
        "shell",
        r#"{"command":"ls","description":"List"}"#,
        Verdict::Deny {
            module: ModuleId::parse("acme-guard").expect("合模块编号的写法"),
            text: "Blocked by acme-guard: no".to_string(),
            human: None,
        },
    );
    let en = title(&same(&stage));
    assert!(en.get("said").is_none(), "没带给人看的说法的不写：{en}");
}

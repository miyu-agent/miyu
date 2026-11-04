//! 一步工具上界面画了的另外三样（施工 SE-1 中下中，照项目主人 09-30「界面上有的导出里才有」）：要人确认的那一次、脚本里调的
//! 工具（`nested`）、工具上挂的提问。三种写法：
//!
//! - `read`：照连接的语言。确认一行（要的访问、怎么定的、理由）；提问一题一行（问的话、几个选项），答了的照「回答了提问」
//!   那条旁白；脚本里调的一个列表（序号、显示名、对象、没成的写一个字和原因），`tools: full` 再把每一件交回的结果折在下面。
//! - `agent`：原文。确认写访问、决定的原样（`execute`、`deny`）；提问在参数原文、回答在结果里，不另写；脚本里调的写工具名、
//!   参数原文、了结成什么样，`full` 再接交回的结果。
//! - 一行（`chat`、`tools: line`）：都接在那一行后面。

use crate::entry::{Approval, Tool};
use crate::nested::{NestedItem, listed};

use miyu_kernel::id::Parent;

use super::options::Tools;
use super::steps::folded_as;
use super::text::{Out, code_block, code_span, one_line};
use super::{Document, Say};

/// `read`：确认那一行。
pub(super) fn approval(approval: &Approval, say: &Say<'_>) -> String {
    let access = approval.access.as_str();
    let access = say
        .view(&format!("core/export/access/{access}"), &[])
        .unwrap_or_else(|| access.to_string());
    let decision = match &approval.decision {
        Some(decision) => say.say(&format!("decision/{}", decision.as_str()), &[]),
        None => say.say("decision/none", &[]),
    };
    let decision = match approval
        .reason
        .as_deref()
        .map(one_line)
        .filter(|r| !r.is_empty())
    {
        Some(reason) => say.say(
            "decision/reason",
            &[("decision", decision), ("reason", reason)],
        ),
        None => decision,
    };
    let fields = [("access", access), ("decision", decision)];
    match approval.nested {
        Some(n) => {
            let mut fields = fields.to_vec();
            fields.push(("n", n.to_string()));
            fields.push(("tool", nested_tool(approval)));
            say.say("approval/nested", &fields)
        }
        None => say.say("approval", &fields),
    }
}

/// `agent`、一行：确认的原文。
pub(super) fn approval_raw(approval: &Approval) -> String {
    let decision = approval
        .decision
        .as_ref()
        .map_or("no decision", |decision| decision.as_str());
    let mut line = match approval.nested {
        Some(n) => format!(
            "approval for nested call #{n} {} ({}): {decision}",
            code_span(&nested_tool(approval)),
            approval.access.as_str()
        ),
        None => format!("approval ({}): {decision}", approval.access.as_str()),
    };
    if let Some(reason) = approval
        .reason
        .as_deref()
        .map(one_line)
        .filter(|r| !r.is_empty())
    {
        line.push_str(&format!(" — {reason}"));
    }
    line
}

/// 脚本里调的、要确认的那一件的工具名：`detail` 的 `tool`。
fn nested_tool(approval: &Approval) -> String {
    approval
        .detail
        .as_ref()
        .and_then(|detail| serde_json::from_str::<serde_json::Value>(detail.get()).ok())
        .and_then(|detail| {
            detail
                .get("tool")
                .and_then(|tool| tool.as_str().map(str::to_string))
        })
        .unwrap_or_default()
}

/// `read`：提问那一步问的那一组，一题一行；没问的没有。
pub(super) fn questions(tool: &Tool) -> Option<String> {
    if tool.questions.is_empty() {
        return None;
    }
    let lines: Vec<String> = tool
        .questions
        .iter()
        .map(|question| {
            let options: Vec<String> = question
                .options
                .iter()
                .map(|choice| one_line(&choice.label))
                .collect();
            match options.is_empty() {
                true => format!("- {}", one_line(&question.question)),
                false => format!(
                    "- {} — {}",
                    one_line(&question.question),
                    options.join(" / ")
                ),
            }
        })
        .collect();
    Some(lines.join("\n"))
}

/// 一行：问的几题。
pub(super) fn questions_line(tool: &Tool) -> Option<String> {
    let asked: Vec<String> = tool
        .questions
        .iter()
        .map(|question| one_line(&question.question))
        .collect();
    (!asked.is_empty()).then(|| format!("asked: {}", asked.join("; ")))
}

/// 挂在 `parent` 下面的脚本里调的工具，照序号。
pub(super) fn items(parent: Option<&Parent>, doc: &Document<'_>) -> Vec<NestedItem> {
    let Some(parent) = parent else {
        return Vec::new();
    };
    match doc.sources.nested.get(parent) {
        Some(events) => listed(parent, events, &[], None, doc.texts),
        None => Vec::new(),
    }
}

/// 没成的那一件写哪个字：`ok` 不写。
fn state_key(state: &str) -> Option<&'static str> {
    match state {
        "error" => Some("state/error"),
        "denied" => Some("state/denied"),
        "cancelled" => Some("state/cancelled"),
        "needs_approval" => Some("state/needs-approval"),
        "running" => Some("state/running"),
        _ => None,
    }
}

/// `read`：脚本里调的工具一个列表，`full` 再把交回的结果一件件折在下面。
pub(super) fn nested(out: &mut Out, parent: Option<Parent>, doc: &Document<'_>, say: &Say<'_>) {
    let items = items(parent.as_ref(), doc);
    if items.is_empty() {
        return;
    }
    let count = u64::try_from(items.len()).unwrap_or(u64::MAX);
    let mut lines = vec![say.count("nested", count, &[])];
    for item in &items {
        let mut line = format!("- #{} {}", item.n, item.title.name);
        if let Some(object) = item
            .title
            .object
            .as_deref()
            .filter(|o| !o.trim().is_empty())
        {
            line.push(' ');
            line.push_str(&code_span(object));
        }
        if let Some(key) = state_key(&item.state) {
            line.push_str(&format!(" · {}", say.say(key, &[])));
        }
        if let Some(reason) = item
            .reason
            .as_deref()
            .map(one_line)
            .filter(|r| !r.is_empty())
        {
            line.push_str(&format!(" · {reason}"));
        }
        lines.push(line);
    }
    out.block(&lines.join("\n"));
    if doc.options.tools == Tools::Full {
        for item in &items {
            if let Some(text) = result(item, doc) {
                out.block(&folded_as(text, say, &format!("#{} ", item.n)));
            }
        }
    }
}

/// `agent`：脚本里调的工具的原文，`full` 再接交回的结果。
pub(super) fn nested_raw(out: &mut Out, parent: Option<Parent>, doc: &Document<'_>) {
    let items = items(parent.as_ref(), doc);
    if items.is_empty() {
        return;
    }
    let mut lines = vec!["Nested calls:".to_string()];
    for item in &items {
        let mut line = format!(
            "- #{} {} {}: {}",
            item.n,
            code_span(&item.name),
            code_span(&item.args),
            item.state
        );
        if let Some(reason) = item
            .reason
            .as_deref()
            .map(one_line)
            .filter(|r| !r.is_empty())
        {
            line.push_str(&format!(" — {reason}"));
        }
        lines.push(line);
    }
    out.block(&lines.join("\n"));
    if doc.options.tools == Tools::Full {
        for item in &items {
            if let Some(text) = result(item, doc) {
                out.block(&format!("Nested call #{} returned:", item.n));
                out.block(&code_block(text, "text"));
            }
        }
    }
}

/// 一行：脚本里调的几件，没成的带上了结成什么样。
pub(super) fn nested_line(parent: Option<Parent>, doc: &Document<'_>) -> Option<String> {
    let items = items(parent.as_ref(), doc);
    let calls: Vec<String> = items
        .iter()
        .map(|item| match item.state.as_str() {
            "ok" => format!("#{} {}", item.n, item.name),
            state => format!("#{} {} ({state})", item.n, item.name),
        })
        .collect();
    (!calls.is_empty()).then(|| format!("nested: {}", calls.join(", ")))
}

/// 一件交回的结果：blob 库里取来的原文。
fn result<'a>(item: &NestedItem, doc: &'a Document<'_>) -> Option<&'a String> {
    item.result
        .as_ref()
        .and_then(|stored| doc.sources.blobs.get(&stored.blob))
}

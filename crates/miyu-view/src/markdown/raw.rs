//! `agent`、`chat` 写一步工具（施工 SE-1 中下中，调研第五节第 2 条「真名、参数原文、结果照她看到的」）：小标题写工具的真名，
//! 没成的在括号里写了结成什么样（原样，`error`、`denied`……）；参数原文放进 `json` 代码块；要人确认的那一次、脚本里调的
//! 工具写原文（`markdown/extras.rs`）；结果照她当时看到的放进 `text` 代码块，`tools: seen` 截到头尾共约 [`MOST`] 个字，
//! `full` 不截；结果里的图写占位。
//!
//! 参数原文就是条目的 `args`：投影照落了盘的 `message.assistant` 里那次调用的 `args` 原样放进来（`projector/blocks.rs`），
//! 不另回日志取。

use crate::entry::{Tool, ToolState};

use super::extras;
use super::options::Tools;
use super::steps::{output, pictures};
use super::text::{Out, code_block, head_and_tail};
use super::{Document, Say};

/// `seen` 时一条结果最多留多少个字：头尾各一半。
pub(super) const MOST: usize = 4000;

/// 一步工具：见模块的说明。
pub(super) fn tool(out: &mut Out, tool: &Tool, doc: &Document<'_>, say: &Say<'_>) {
    let mut head = format!("### Tool: {}", tool.name);
    if let Some(state) = state(tool.state) {
        head.push_str(&format!(" ({state})"));
    }
    out.block(&head);
    if !tool.args.trim().is_empty() {
        out.block(&code_block(&tool.args, "json"));
    }
    if let Some(approval) = &tool.approval {
        out.block(&capital(&extras::approval_raw(approval)));
    }
    extras::nested_raw(out, tool.call.map(miyu_kernel::id::Parent::Call), doc);
    if let Some(text) = output(tool, doc).filter(|text| !text.trim().is_empty()) {
        let text = match doc.options.tools {
            Tools::Full => std::borrow::Cow::Borrowed(text.as_str()),
            Tools::Seen | Tools::Line => head_and_tail(text, MOST),
        };
        out.block(&code_block(&text, "text"));
    }
    let pictures = pictures(tool, doc, say);
    if !pictures.is_empty() {
        out.block(&pictures.join("\n"));
    }
}

/// 没成的了结成什么样，原样；成了的、还没结果的没有。
pub(super) fn state(state: ToolState) -> Option<&'static str> {
    match state {
        ToolState::Error => Some("error"),
        ToolState::Denied => Some("denied"),
        ToolState::Cancelled => Some("cancelled"),
        ToolState::Skipped => Some("skipped"),
        ToolState::Ok | ToolState::Running | ToolState::Preparing => None,
    }
}

/// 头一个字母大写。
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

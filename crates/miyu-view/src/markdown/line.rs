//! 一步工具写一行（施工 SE-1 中下中，`chat` 预设、`tools: line`）：做了什么、成没成，出错的带上错误，改了几行；要人确认的
//! 那一次、脚本里调的工具、提问的题接在后面（`markdown/extras.rs`）。连着的几步是同一个列表。
//!
//! - `read`：显示名、对象、结果那一句、没成的写一个字，照连接的语言。
//! - `agent`、`chat`：工具的真名、对象、没成的原样（`error`、`denied`……），确认、脚本里调的照原文。

use crate::entry::{Tool, ToolState};

use super::extras;
use super::options::Purpose;
use super::raw;
use super::steps::{output, said};
use super::text::{Out, code_span, one_line};
use super::{Document, Say};

/// 出错带的那一句最多几个字。
const ERROR_MOST: usize = 200;

/// 一步工具一行：见模块的说明。
pub(super) fn tool(out: &mut Out, tool: &Tool, doc: &Document<'_>, say: &Say<'_>) {
    let parent = tool.call.map(miyu_kernel::id::Parent::Call);
    let mut parts: Vec<String> = Vec::new();
    let mut head = match doc.options.purpose {
        Purpose::Read => {
            let mut name = tool.title.name.clone();
            if let Some(via) = &tool.title.via {
                name.push_str(&format!(" ({via})"));
            }
            format!("- Tool: {name}")
        }
        Purpose::Agent | Purpose::Chat => format!("- Tool: {}", code_span(&tool.name)),
    };
    if let Some(object) = tool
        .title
        .object
        .as_deref()
        .filter(|o| !o.trim().is_empty())
    {
        head.push(' ');
        head.push_str(&code_span(object));
    }
    parts.push(head);
    match doc.options.purpose {
        Purpose::Read => {
            parts.extend(said(tool, say));
            if let Some(approval) = &tool.approval {
                parts.push(extras::approval(approval, say));
            }
            let nested = extras::items(parent.as_ref(), doc);
            if !nested.is_empty() {
                let count = u64::try_from(nested.len()).unwrap_or(u64::MAX);
                parts.push(say.count("nested", count, &[]));
            }
        }
        Purpose::Agent | Purpose::Chat => {
            if let Some(state) = raw::state(tool.state) {
                parts.push(state.to_string());
            }
            if let Some(diff) = tool.diff {
                parts.push(format!("+{} −{}", diff.added, diff.removed));
            }
            if let Some(approval) = &tool.approval {
                parts.push(extras::approval_raw(approval));
            }
            parts.extend(extras::nested_line(parent, doc));
            parts.extend(extras::questions_line(tool));
        }
    }
    let mut line = parts.join(" · ");
    if tool.state == ToolState::Error
        && let Some(error) = output(tool, doc)
            .map(|text| one_line(text))
            .filter(|e| !e.is_empty())
    {
        line.push_str(": ");
        line.push_str(&short(&error));
    }
    out.item(&line);
}

/// 最多 [`ERROR_MOST`] 个字，多了截掉写省略号。
fn short(text: &str) -> String {
    match text.char_indices().nth(ERROR_MOST) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_string(),
    }
}

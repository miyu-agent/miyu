//! 她的一步：思考折起来；工具照开关分三种写法：一行（`markdown/line.rs`）、`read` 的（这里）、`agent`、`chat` 的
//! （`markdown/raw.rs`）。
//!
//! `read` 的一步工具：小标题写显示名、对象，下一行结果那一句、出错没出错、加减几行；要人确认的那一次一行；派子代理的交代
//! 折起来；提问的那一组题；输出照她当时看到的，长的折起来；脚本里调的工具一个列表；结果里的图写占位。

use serde_json::Value;

use crate::entry::{Thought, Tool, ToolState};
use crate::summary::clock;
use crate::words::ToolKind;

use super::extras;
use super::options::{Purpose, Tools};
use super::text::{Out, code_block, code_span, details, prose};
use super::{Document, Say};

/// 输出超过这么多行折起来。
const FOLD_LINES: usize = 20;

/// 输出超过这么多字节折起来。
const FOLD_BYTES: usize = 2000;

/// 一步思考：折起来，那一行写想了多久（同收起那一行的「思考了 …」），没记下用时的只写「思考」。
pub(super) fn thought(out: &mut Out, thought: &Thought, say: &Say<'_>) {
    let summary = thought
        .took_ms
        .and_then(|ms| say.view("core/view/summary/thought-for", &[("elapsed", clock(ms))]))
        .unwrap_or_else(|| say.say("thinking", &[]));
    out.block(&details(&summary, &prose(&thought.text, true)));
}

/// 一步工具：照开关挑写法。
pub(super) fn tool(out: &mut Out, tool: &Tool, doc: &Document<'_>, say: &Say<'_>) {
    match (doc.options.tools, doc.options.purpose) {
        (Tools::Line, _) => super::line::tool(out, tool, doc, say),
        (_, Purpose::Read) => read(out, tool, doc, say),
        (_, Purpose::Agent | Purpose::Chat) => super::raw::tool(out, tool, doc, say),
    }
}

/// `read` 的一步工具：见模块的说明。
fn read(out: &mut Out, tool: &Tool, doc: &Document<'_>, say: &Say<'_>) {
    let title = &tool.title;
    let mut head = format!("### Tool: {}", title.name);
    if let Some(via) = &title.via {
        head.push_str(&format!(" ({via})"));
    }
    if let Some(object) = title.object.as_deref().filter(|o| !o.trim().is_empty()) {
        head.push(' ');
        head.push_str(&code_span(object));
    }
    out.block(&head);
    let line = said(tool, say);
    if !line.is_empty() {
        out.block(&line.join(" · "));
    }
    if let Some(approval) = &tool.approval {
        out.block(&extras::approval(approval, say));
    }
    if doc.texts.kinds.of(&tool.name) == Some(ToolKind::Agent)
        && let Some(prompt) = brief(&tool.args)
    {
        out.block(&details(&say.say("brief", &[]), &prose(&prompt, true)));
    }
    if let Some(questions) = extras::questions(tool) {
        out.block(&questions);
    }
    if let Some(output) = output(tool, doc).filter(|output| !output.trim().is_empty()) {
        out.block(&folded(output, say));
    }
    extras::nested(out, tool.call.map(miyu_kernel::id::Parent::Call), doc, say);
    let pictures = pictures(tool, doc, say);
    if !pictures.is_empty() {
        out.block(&pictures.join("\n"));
    }
}

/// 结果那一句、出错没出错、加减几行：照连接的语言。
pub(super) fn said(tool: &Tool, say: &Say<'_>) -> Vec<String> {
    let mut line: Vec<String> = tool.title.said.iter().cloned().collect();
    if let Some(state) = state(tool.state) {
        line.push(say.say(state, &[]));
    }
    if let Some(diff) = tool.diff {
        line.push(format!("+{} −{}", diff.added, diff.removed));
    }
    line
}

/// 她当时看到的结果：切过的照日志里的全文，没切过的条目里整份有。
pub(super) fn output<'a>(tool: &'a Tool, doc: &'a Document<'_>) -> Option<&'a String> {
    match (tool.output_cut, tool.call) {
        (true, Some(call)) => doc.sources.outputs.get(&call).or(tool.output.as_ref()),
        _ => tool.output.as_ref(),
    }
}

/// 结果里的图：一张一行，`assets: files` 取得到的写成链接，别的写占位。
pub(super) fn pictures(tool: &Tool, doc: &Document<'_>, say: &Say<'_>) -> Vec<String> {
    tool.images
        .iter()
        .map(|picture| {
            let about = format!("{}×{}", picture.width, picture.height);
            let link = doc.files.and_then(|files| {
                let named = (None, picture.media_type.as_str());
                files.link(&picture.blob, true, Some(about.clone()), named)
            });
            let shown = link.unwrap_or_else(|| say.say("image", &[("about", about)]));
            format!("- {shown}")
        })
        .collect()
}

/// 一段输出放进代码块，长的折起来，那一行写几行。
pub(super) fn folded(text: &str, say: &Say<'_>) -> String {
    folded_as(text, say, "")
}

/// 同 [`folded`]，折起来那一行前面加 `prefix`（脚本里调的工具写 `#序号 `）。
pub(super) fn folded_as(text: &str, say: &Say<'_>, prefix: &str) -> String {
    let text = text.trim_end_matches(['\n', '\r']);
    let lines = text.lines().count();
    let block = code_block(text, "text");
    match lines > FOLD_LINES || text.len() > FOLD_BYTES || !prefix.is_empty() {
        true => {
            let count = u64::try_from(lines).unwrap_or(u64::MAX);
            details(
                &format!("{prefix}{}", say.count("output", count, &[])),
                &block,
            )
        }
        false => block,
    }
}

/// 出错、被拒、打断、跳过的写一个字；成了的、还没结果的不写。
pub(super) fn state(state: ToolState) -> Option<&'static str> {
    match state {
        ToolState::Error => Some("state/error"),
        ToolState::Denied => Some("state/denied"),
        ToolState::Cancelled => Some("state/cancelled"),
        ToolState::Skipped => Some("state/skipped"),
        ToolState::Ok | ToolState::Running | ToolState::Preparing => None,
    }
}

/// 派子代理那一步的交代：参数里的 `prompt`。
fn brief(args: &str) -> Option<String> {
    let args: Value = serde_json::from_str(args).ok()?;
    let prompt = args.get("prompt")?.as_str()?;
    (!prompt.trim().is_empty()).then(|| prompt.to_string())
}

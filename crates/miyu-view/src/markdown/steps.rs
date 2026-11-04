//! 她的一步：思考折起来；工具写显示名、对象、结果那一句，派子代理的带交代，输出照她当时看到的，长的折起来。

use serde_json::Value;

use crate::entry::{Thought, Tool, ToolState};
use crate::summary::clock;
use crate::words::ToolKind;

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

/// 一步工具：小标题 `Tool: <显示名>`，后面是对象；下一行结果那一句、出错没出错、加减几行；派子代理的交代折起来；输出、图。
pub(super) fn tool(out: &mut Out, tool: &Tool, doc: &Document<'_>, say: &Say<'_>) {
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
    let mut line: Vec<String> = title.said.iter().cloned().collect();
    if let Some(state) = state(tool.state) {
        line.push(say.say(state, &[]));
    }
    if let Some(diff) = tool.diff {
        line.push(format!("+{} −{}", diff.added, diff.removed));
    }
    if !line.is_empty() {
        out.block(&line.join(" · "));
    }
    if doc.texts.kinds.of(&tool.name) == Some(ToolKind::Agent)
        && let Some(prompt) = brief(&tool.args)
    {
        out.block(&details(&say.say("brief", &[]), &prose(&prompt, true)));
    }
    let output = match (tool.output_cut, tool.call) {
        (true, Some(call)) => doc.sources.outputs.get(&call).or(tool.output.as_ref()),
        _ => tool.output.as_ref(),
    };
    if let Some(output) = output.filter(|output| !output.trim().is_empty()) {
        out.block(&folded(output, say));
    }
    let pictures: Vec<String> = tool
        .images
        .iter()
        .map(|picture| {
            let about = format!("{}×{}", picture.width, picture.height);
            format!("- {}", say.say("image", &[("about", about)]))
        })
        .collect();
    if !pictures.is_empty() {
        out.block(&pictures.join("\n"));
    }
}

/// 一段输出放进代码块，长的折起来，那一行写几行。
pub(super) fn folded(text: &str, say: &Say<'_>) -> String {
    let text = text.trim_end_matches(['\n', '\r']);
    let lines = text.lines().count();
    let block = code_block(text, "text");
    match lines > FOLD_LINES || text.len() > FOLD_BYTES {
        true => {
            let count = u64::try_from(lines).unwrap_or(u64::MAX);
            details(&say.count("output", count, &[]), &block)
        }
        false => block,
    }
}

/// 出错、被拒、打断、跳过的写一个字；成了的、还没结果的不写。
fn state(state: ToolState) -> Option<&'static str> {
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

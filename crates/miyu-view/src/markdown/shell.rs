//! 人用 `!` 跑的那条命令（施工 SH-2，设计 57 第四节第 4 条、设计 58 第一节）：那条 `user` 带 `shell` 的，正文不当一句话写，
//! 写成 `console` 代码块里的 `$ <命令>`；下一行怎么结束的（退出码或信号、异常结束的原因、还在跑）和用时；输出照工具的写法：
//! `read` 长的折起来，`agent`、`chat` 照 `tools` 截头尾，`tools: line` 不写输出。

use std::borrow::Cow;

use crate::entry::Shell;
use crate::summary::clock;

use super::options::Tools;
use super::raw::MOST;
use super::steps::folded;
use super::text::{Out, code_block, head_and_tail};
use super::{Document, Purpose, Say};

/// 一条命令：见模块的说明。
pub(super) fn write(
    out: &mut Out,
    command: &str,
    shell: &Shell,
    doc: &Document<'_>,
    say: &Say<'_>,
) {
    out.block(&code_block(&format!("$ {command}"), "console"));
    let line = ended(shell, say);
    if !line.is_empty() {
        out.block(&line.join(" · "));
    }
    let Some(text) = shell.text.as_deref().filter(|text| !text.trim().is_empty()) else {
        return;
    };
    match (doc.options.tools, doc.options.purpose) {
        (Tools::Line, _) => {}
        (_, Purpose::Read) => out.block(&folded(text, say)),
        (Tools::Full, _) => out.block(&code_block(text, "text")),
        (Tools::Seen, _) => {
            let text: Cow<'_, str> = head_and_tail(text, MOST);
            out.block(&code_block(&text, "text"));
        }
    }
}

/// 怎么结束的、用了多久：还在跑的只写「还在跑」；异常结束的先写原因，再写退出码或信号。
fn ended(shell: &Shell, say: &Say<'_>) -> Vec<String> {
    if shell.running {
        return vec![say.say("shell/running", &[])];
    }
    let mut line = Vec::new();
    if let Some(key) = shell.reason.as_deref().and_then(reason) {
        line.push(say.say(key, &[]));
    }
    if let Some(code) = shell.exit {
        line.push(say.say("shell/exit", &[("code", code.to_string())]));
    }
    if let Some(signal) = shell.signal {
        line.push(say.say("shell/signal", &[("signal", signal.to_string())]));
    }
    if let Some(ms) = shell.duration_ms {
        line.push(clock(ms));
    }
    line
}

/// 异常结束的原因到那一句的编号；不认得的不写（视图流照事件原样给，以后多一种也不至于写出编号）。
fn reason(reason: &str) -> Option<&'static str> {
    match reason {
        "timed_out" => Some("shell/timed-out"),
        "interrupted" => Some("shell/interrupted"),
        "aborted" => Some("shell/aborted"),
        _ => None,
    }
}

//! 人用 `!` 跑的命令（施工 SH-1 上，设计 57 第四节第 3 条，`docs/blueprint/kernel/request.md`「人跑的命令」）：写了 `shell`
//! 的 `message.user` 和它的 `shell.ran` 在那句话的位置渲染成一块：标签带怎么结束的，里面是那一行命令、原样的输出（截过的中间
//! 已经有一行省了多少）、退出码或信号，只中和提前写出的本标签收尾。只陈述人跑了什么、输出什么。

use std::collections::BTreeMap;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, MessageUser, ShellRan};
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;

use crate::jobs::fill;
use crate::tag::defuse;
use crate::texts::{JobTexts, Texts};

/// 有效历史里每一句的 `shell.ran`，照那句话的序号。
pub(crate) fn rans(history: &History) -> BTreeMap<Seq, &ShellRan> {
    history
        .events()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ShellRan(ran) => Some((ran.message, ran)),
            _ => None,
        })
        .collect()
}

/// 人用 `!` 打的那一句 `message` 渲染成的块：和它跑完的样子 `ran` 拼成一块；以前造的快照照原样放那一行。
pub(crate) fn spoken(message: &MessageUser, ran: Option<&ShellRan>, texts: &Texts) -> Vec<Block> {
    let block = texts
        .jobs
        .as_ref()
        .and_then(|jobs| block(&message.text(), ran, jobs));
    match block {
        Some(text) => vec![Block::Text(Text { text })],
        None => message.blocks.clone(),
    }
}

/// 命令 `command` 和它跑完的样子 `ran` 那一块。还没有 `shell.ran` 的（载入时补之前读到的旧日志）照没跑完写；以前造的快照没有
/// 这几份字的，没有：照原样放那一行。
fn block(command: &str, ran: Option<&ShellRan>, texts: &JobTexts) -> Option<String> {
    if texts.shell_close.is_empty() {
        return None;
    }
    let reason = match ran {
        Some(ran) => ran
            .reason
            .as_ref()
            .map_or("exited", |reason| reason.as_str()),
        None => "aborted",
    };
    let closes = [texts.shell_close.as_str()];
    let mut block = fill(&texts.shell_open, &[("reason", reason)]);
    // 命令行照人打的原样放（引号、换行都不转义），只中和提前写出的收尾：模板的转义是给标签里的字段用的。
    let fields = BTreeMap::from([("command", command)]);
    let line = texts
        .shell_command
        .fill(&fields, |value| defuse(value, &closes).into_owned());
    block.push_str(&line.unwrap_or_default());
    let Some(ran) = ran else {
        block.push_str(&texts.shell_close);
        return Some(block);
    };
    if !ran.text.is_empty() {
        block.push_str(&defuse(&ran.text, &closes));
        if !ran.text.ends_with('\n') {
            block.push('\n');
        }
    }
    let ended = [
        (&texts.command_exit, "code", ran.exit.map(|n| n.to_string())),
        (
            &texts.command_signal,
            "signal",
            ran.signal.map(|n| n.to_string()),
        ),
    ];
    for (template, field, value) in ended {
        if let Some(value) = value {
            block.push_str(&fill(template, &[(field, &value)]));
        }
    }
    block.push_str(&texts.shell_close);
    Some(block)
}

#[cfg(test)]
mod tests;

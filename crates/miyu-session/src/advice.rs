//! 顾问会话（设计 `69-旗舰顾问.md`，施工 ADV-1）：`advisor` 问旗舰池里的模型开的子会话。和子代理同一套（`session.created` 带
//! `parent`、`depth`，用量照子会话记），只是：快照是裸的、system 换成 `core/advisor/system.txt`（没有工具、没有人格，回合开始
//! 不注入事实），`origin` 记 [`ADVISOR_ORIGIN`]，不是父会话的任务（没有 `job.started`，不向上回报）。

use miyu_kernel::id::{CallId, CommandId, SessionId};

/// 顾问会话的 `session.created` 的 `origin`（设计 69 第三节第 5 条「子会话的种类 `advisor`」）：会话表照它找父会话最近一次的
/// 顾问会话、收空子会话时不当派到一半的子代理。
pub const ADVISOR_ORIGIN: &str = "advisor";

/// 命令编号里顾问那一段：`<父会话>/advisor/<调用>`。
const SEGMENT: &str = "advisor";

/// 照 `origin` 是不是顾问会话。
pub fn advising(origin: Option<&str>) -> bool {
    origin == Some(ADVISOR_ORIGIN)
}

/// 父会话 `parent` 的调用 `call` 问顾问的命令编号：`<父会话>/advisor/<调用>`。父会话的编号整个数据根里不重，调用编号一个会话里
/// 不重，所以它在哪儿都不重；长度离命令编号的 128 个字节远着。
pub(crate) fn command(parent: &SessionId, call: CallId) -> CommandId {
    CommandId::parse(&format!("{parent}/{SEGMENT}/{call}"))
        .unwrap_or_else(|e| unreachable!("会话编号、调用编号都短，合命令编号的写法：{e}"))
}

/// 造它的命令编号 `command` 是不是父会话 `parent` 问顾问的（[`command`]）：顾问会话不是任务，读不出任务编号是对的，不记 `WARN`
/// （`crate::report::Upstream::of`）。
pub(crate) fn is_advice(parent: &SessionId, command: &CommandId) -> bool {
    command
        .as_str()
        .strip_prefix(parent.as_str())
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.strip_prefix(SEGMENT))
        .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests;

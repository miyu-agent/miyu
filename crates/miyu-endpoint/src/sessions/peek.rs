//! 主人只读地看场所会话（施工 VW-1；2026-10-23 项目主人定场所会话只读、不从头上交互，2026-11-12 定「查看会话只看不发」）：
//! 群、陌生人的私聊这类关着的场所会话，人的头（没有包的连接）照 [`READS`] 里的方法看得到，表外的照旧当没有这个会话。
//! 第 4 道（`connection/checks.rs`）、`blob.get`（`attach/reach.rs`）、`session.shown` 都照 [`peeks`] 判，只这一份。
//! 平台桥、扩展的规矩不变：它们不走这里。`schedule.add` 的 `target.session`、HK-2 挑 `tool.after` 收件人照的
//! [`super::reaches`] 也不变。

use crate::hello::Caller;

/// 人的头对关着的场所会话也能调的方法，默认拒：只读，不改会话的状态、日志，不往会话里交东西。加新方法要先判它是不是读，
/// 拿不准的不放（`tests/owner_views_venue.rs` 照协议的方法表逐个守着）。
///
/// - `subscribe`、`unsubscribe`：订、退这个连接自己的事件流、视图流（连补发），会话里什么都不变。
/// - `view.page`、`view.detail`：读日志里的历史、一次调用的完整结果、一次压缩的摘要。
/// - `session.prompts`：只读日志列问过的话，不载入会话。
/// - `session.tools`、`session.context`、`session.usage_parts`：照这一刻的配置、日志、用量账本算，不影响会话。
/// - `session.quota`：问会话接下来请求的那一家的额度（施工 QU-1），不影响会话。
/// - `job.output`：读后台命令的输出。
/// - `blob.get`、`session.shown`：读会话里的附件、展示过的文件（各有自己的判法，拒的说法照旧）。
pub(crate) const READS: &[&str] = &[
    "subscribe",
    "unsubscribe",
    "view.page",
    "view.detail",
    "session.prompts",
    "session.tools",
    "session.context",
    "session.usage_parts",
    "session.quota",
    "job.output",
    "blob.get",
    "session.shown",
];

/// 连接 `caller` 调 `method` 时只读地看得到碰不到的会话：人的头（没有包）、方法在 [`READS`] 里。
pub(crate) fn peeks(caller: &Caller, method: &str) -> bool {
    caller.package.is_none() && READS.contains(&method)
}

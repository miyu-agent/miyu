//! 人在头的输入框里用 `!` 跑的一条命令跑完了（施工 SH-1 上，设计 57 第四节第 2 条）：`message.user` 写了 `shell` 的那一句，
//! 核心在她听到它的那一刻跑（`session/shell.rs`），结果记一条 `shell.ran`。渲染时和那句话拼在一起（`miyu-assemble`）。

use serde::{Deserialize, Serialize};

use crate::id::Seq;
use crate::text_enum::text_enum;

/// `shell.ran`：人打的一条命令跑完了。`by` 是内核，`cause` 照那句话的。不带效果：命令改了的文件照「外面改的」算。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellRan {
    /// 哪一句：那条 `message.user` 的序号。
    pub message: Seq,
    /// 退出码，照系统交回的原样：带符号。被信号杀掉的、到时、被打断的、没跑完的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    /// 杀掉它的信号的编号（Unix）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<u32>,
    /// 从起进程到结束的毫秒数；没跑起来的（载入时补的）是 0。
    pub duration_ms: u64,
    /// 输出：标准输出、标准错误合在一起，`\r\n` 换成 `\n`，太长的照前台 `shell` 的截法留头尾、中间一行写省了多少。
    pub text: String,
    /// 截掉了多少个字（Unicode 字符），没截的是 0。
    pub omitted: u64,
    /// 异常结束的为什么；自己退出的（被信号杀掉的也算）没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ShellReason>,
}

text_enum!(
    /// 人打的命令为什么没跑完。
    ShellReason {
        /// 跑到了时限（120 秒），整组杀掉了。
        TimedOut = "timed_out",
        /// 人打断了，整组杀掉了；排着时被打断、留在日志里的那几条也是，没跑过。
        Interrupted = "interrupted",
        /// 核心重启了或者崩了，没跑完、也不重跑：重启时记，崩了的载入时补。
        Aborted = "aborted",
    }
);

#[cfg(test)]
mod tests;

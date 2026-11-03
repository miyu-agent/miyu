//! 一件后台任务了结了的那一条旁白的几格（施工 V-D 从 `entry.rs` 挪出来：那一份加了读回来的写法以后过了行数上限）。

use serde::{Deserialize, Serialize};

use miyu_kernel::event::{ChildReason, JobKind, JobReason};
use miyu_kernel::id::{ContentHash, JobId};

use super::Nested;

/// 一件后台任务了结了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobDone {
    /// 任务编号。
    pub job: JobId,
    /// 后台命令还是子代理（不叫 `what`：旁白的种类占了那个名字）。
    pub job_kind: JobKind,
    /// 标题：派它那一步的描述。
    pub title: String,
    /// 记号：`done`、`failed`、`stopped`。
    pub mark: Mark,
    /// 回报里的原因，原样。
    pub reason: JobEnd,
    /// 子代理的报告全文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    /// 后台命令：派它那次调用的命令原文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// 后台命令的输出。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Output>,
    /// 后台命令的退出码（施工 9-8 三补）：照回报，没有的不写。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// 杀掉后台命令的信号（施工 9-8 三补）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u32>,
    /// 后台命令从起到结束的毫秒数（施工 9-8 三补）：照回报，核心崩了补的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
    /// 后台命令下面脚本里调过的工具（施工 M-5 上）：最后那一份。没调过的不写。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested: Option<Nested>,
}

/// 监视的命令出的一批行（施工 MN-1 下，设计 56 第四节第 4 条，`job.lines`）：头照回报那样画一行，点开看全文。不是了结，
/// 状态栏那一行照旧在跑。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchedLines {
    /// 任务编号。
    pub job: JobId,
    /// 标题：派它那一步的描述。
    pub title: String,
    /// 派它那次调用的命令原文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// 这一批的行，照事件原样（截过的中间有一行省了多少）。
    pub text: String,
    /// 这一批原来几行。
    pub lines: u64,
    /// 截掉了几个字。
    pub omitted: u64,
}

/// 后台任务了结的原因：后台命令的、子代理的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JobEnd {
    /// 后台命令。
    Command(JobReason),
    /// 子代理。
    Agent(ChildReason),
}

/// 后台任务了结的记号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    /// 做完了。
    Done,
    /// 失败了。
    Failed,
    /// 停掉了。
    Stopped,
}

/// 后台命令的输出。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    /// 内容的哈希。
    pub blob: ContentHash,
    /// 多少个字。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chars: Option<u64>,
}

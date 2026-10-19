//! 快照里装的几组随核心附带的字（`resources/core/`）：压缩、权限、回合没走完、替工具写的（从 `snapshot.rs` 挪出来：那边到了
//! 行数上限）。

use serde::{Deserialize, Serialize};

use crate::rebuild::RebuildTexts;
use crate::shorten::ShortenTexts;

/// 压缩的几句（施工 6-2 上）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionTexts {
    /// 摘要指令的正文（`summarize-task.txt`）。
    pub summarize_task: String,
    /// 手动压缩附了要求的，要求前面那一行（`summarize-instructions.txt`，施工 6-8）。以前造的快照里没有，读成空的。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summarize_instructions: String,
    /// 摘要指令的最后一句（`summarize-end.txt`，施工 6-8 从正文里拆出来）。以前造的快照里没有，读成空的：那时的正文里
    /// 本来就带着这一句，拼出来一字不差。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summarize_end: String,
    /// 压后重建的字（施工 6-5）。以前造的快照里没有，读成没有：不写那几段、不重读。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<RebuildTexts>,
    /// 截短重试的字（施工 6-6 中）。以前造的快照里没有，读成没有：不截短。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shorten: Option<ShortenTexts>,
    /// 隔离式那一句 system（`summarize-system.txt`，施工 6-6 下）。以前造的快照里没有，读成没有：摘要回复里调了工具照
    /// 失败算。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summarize_system: Option<String>,
}

/// 权限策略拒绝时写给她的两句（施工 4-3 下）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionTexts {
    /// 碰到了数据根（`forbidden.txt`）。
    pub forbidden: String,
    /// 路径换不成真实的位置（`unresolvable.txt`）。
    pub unresolvable: String,
}

/// 回合没走完的几句，照原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEndedTexts {
    /// 被人打断（`interrupted.txt`）。
    pub interrupted: String,
    /// 出错（`error.txt`）。
    pub error: String,
    /// 走到了步数上限（`step_limit.txt`）。
    pub step_limit: String,
    /// 核心崩了，没走完（`aborted.txt`）。
    pub aborted: String,
    /// 被有计划的重启打断（`restarted.txt`）。
    pub restarted: String,
}

/// 内核替工具写给模型的几句，名字照 `resources/core/tool-results/` 里的文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultTexts {
    /// 没有这件工具（`unknown.txt`）。
    pub unknown: String,
    /// 参数不是 JSON 对象（`not-an-object.txt`）。
    pub not_an_object: String,
    /// 还没开始就被打断（`cancelled-before.txt`）。
    pub cancelled_before: String,
    /// 跑到一半被打断（`cancelled-running.txt`）。
    pub cancelled_running: String,
    /// 急着插话，这一步没跑的（`skipped.txt`）。
    pub skipped: String,
    /// 只读时拦下写入的（`read-only.txt`）。
    pub read_only: String,
    /// 人拒绝了（`denied.txt`）。
    pub denied: String,
    /// 人拒绝了，还说了理由（`denied-with-reason.txt`）。
    pub denied_with_reason: String,
    /// 要确认却没人能确认（`unattended.txt`）。
    pub unattended: String,
    /// 问人的时候被打断（`question-interrupted.txt`）。
    pub question_interrupted: String,
    /// 问的题作废了（`question-voided.txt`）。
    pub question_voided: String,
    /// 要问人却没人能回答（`question-unattended.txt`）。
    pub question_unattended: String,
    /// 有计划的重启打断了调用（`restarted.txt`）。
    pub restarted: String,
    /// 快照里有、核心的目录里没有的工具（`unavailable.txt`，施工 4-2）：执行器写。这一格以前造的会话
    /// 里没有，读成空的：那时的会话没有工具，用不到它。
    #[serde(default)]
    pub unavailable: String,
    /// 工具执行时崩了（`crashed.txt`，施工 4-2）：执行器写。读不到的同上。
    #[serde(default)]
    pub crashed: String,
    /// 快照里有、随包卸掉了的工具（`uninstalled.txt`，施工 F-5 中，设计 30 第九节）：执行器写。以前造的快照里没有，读成空的，
    /// 照 `unavailable` 说。
    #[serde(default)]
    pub uninstalled: String,
}

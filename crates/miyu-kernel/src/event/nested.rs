//! `tool.nested`：脚本里的一次 `miyu call` 了结了（施工 M-1，设计 `39-脚本调工具.md` 第四节、第七节，蓝图
//! `kernel/events-bodies.md`「`tool.nested`」）。
//!
//! 了结时记一条，开始时不记；崩了以后不补。`by` 是内核，挂在谁下面看 [`ToolNested::id`]。不进上下文：组装请求时跳过。
//! 收不收由账本查（`ledger.rs`）：父级还活着才收，父级是调用的要在那次调用所在的回合里。

use serde::{Deserialize, Serialize};

use crate::id::{ContentHash, NestedId};
use crate::text_enum::text_enum;

/// `tool.nested`：一次嵌套调用的了结。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolNested {
    /// `<父级>#<序号>`：挂在哪一次调用或哪一个任务下面、是第几个。
    pub id: NestedId,
    /// 调的哪件工具。
    pub name: String,
    /// 修正过的参数，JSON 原文。
    pub args: String,
    /// 了结成什么样。
    pub status: NestedStatus,
    /// 执行用了多少毫秒，执行器量的。没真执行过的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 交给脚本的那份结果存成的 blob。没执行过的、没存下来的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ContentHash>,
    /// 那份结果有多少字节，和 `result` 一起有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// 拒绝的那一句，照直接调用的原话。只跟着 `denied`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

text_enum!(
    /// 嵌套调用了结成什么样（设计 39 第七节第 1 条）。
    NestedStatus {
        /// 成功：执行器交回的。
        Ok = "ok",
        /// 工具执行了，但是出了错：执行器交回的。
        Error = "error",
        /// 执行前的链、内核拦下了。
        Denied = "denied",
        /// 链判出要问人：不问，当场了结（设计 39 MC4）。
        NeedsApproval = "needs_approval",
        /// 父级先结束了，还在跑的、还在排队的。
        Cancelled = "cancelled",
    }
);

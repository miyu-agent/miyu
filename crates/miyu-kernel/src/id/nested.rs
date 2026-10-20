//! 嵌套调用编号：`<父级>#<序号>`（施工 M-1，设计 `39-脚本调工具.md` 第四节第 1 条，蓝图 `kernel/ids.md`「嵌套调用编号」、
//! 第 27 到 29 条）。
//!
//! 父级是前台那次 `shell` 调用的 [`CallId`]，或者后台命令的 [`JobId`]；序号从 1 数，是这个父级下面的第几个嵌套调用。用 `#`
//! 不用 `.`：任务编号里的 `.` 已经是子会话的一层（`j7.3` 是 `j7` 派的第 3 个任务），照最后一个 `#` 切就不会撞。

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{CallId, JobId, decimal};
use crate::format_error::FormatError;

/// 嵌套调用挂在谁下面：前台的一次调用，或者一个后台任务。排序时调用在任务前面。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Parent {
    /// 前台的那次调用：它记结果之前，挂在下面的都要了结。
    Call(CallId),
    /// 后台命令的任务：它报告之前，挂在下面的都要了结。
    Job(JobId),
}

impl fmt::Display for Parent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Parent::Call(call_id) => call_id.fmt(f),
            Parent::Job(job) => job.fmt(f),
        }
    }
}

/// 嵌套调用编号，写成 `call_12_1#3`、`j7#3`。JSON 里是字符串；排序先照父级，再照序号。
///
/// 序号从 1 起：只有 [`NestedId::new`]、[`NestedId::parse`] 造得出来，都守着这一条。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NestedId {
    parent: Parent,
    n: u32,
}

impl NestedId {
    /// 父级下面的第 `n` 个。从 1 数起，给 0 返回 `None`。
    pub fn new(parent: Parent, n: u32) -> Option<NestedId> {
        (n >= 1).then_some(NestedId { parent, n })
    }

    /// 挂在谁下面。
    pub fn parent(&self) -> &Parent {
        &self.parent
    }

    /// 这个父级下面的第几个，从 1 数起。
    pub fn n(&self) -> u32 {
        self.n
    }

    /// 读 `call_12_1#3`、`j7.3#2` 这样的写法。
    ///
    /// # Errors
    ///
    /// 只认内核自己写出去的样子：没有 `#` 的；`#` 前面既不是调用编号也不是任务编号的；序号不是从 1 起的十进制数、超出
    /// `u32` 的，返回 [`FormatError`]。
    pub fn parse(text: &str) -> Result<NestedId, FormatError> {
        let bad = |why| FormatError::new("nested id", text, why);
        let (parent, n) = text
            .rsplit_once('#')
            .ok_or_else(|| bad("write it as <call id or job id>#<n>"))?;
        let n = decimal(n)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| bad("n must be a decimal number from 1"))?;
        let parent = match parent.starts_with("call_") {
            true => CallId::parse(parent).map(Parent::Call),
            false => JobId::parse(parent).map(Parent::Job),
        }
        .map_err(|_| bad("parent must be a call id or a job id"))?;
        NestedId::new(parent, n).ok_or_else(|| bad("n must be a decimal number from 1"))
    }
}

impl fmt::Display for NestedId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.parent, self.n)
    }
}

impl Serialize for NestedId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for NestedId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        NestedId::parse(&String::deserialize(d)?).map_err(D::Error::custom)
    }
}

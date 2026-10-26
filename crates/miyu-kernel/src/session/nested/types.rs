//! 嵌套调用和执行器之间的几样（施工 M-1，`docs/blueprint/kernel/session.md`「嵌套调用」）：执行器交来的回报，交给执行器的
//! 动作，答脚本那头的话。

use serde::Serialize;

use super::super::input::Verdict;
use crate::event::{NestedStatus, Permission};
use crate::id::{ContentHash, NestedId};
use crate::raw::RawJson;
use crate::time::Timestamp;

/// 执行器交来的嵌套调用的三种回报（[`crate::session::Input::Nested`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NestedInput {
    /// 脚本里来了一次 `miyu call`。
    Call {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 执行器在父级下面编的号。
        id: NestedId,
        /// 工具名。
        name: String,
        /// 参数原文。
        args: String,
        /// 核心自带的命令（会话里的 `miyu pkg`）经通道调的（施工 BS-1 中，设计 `44-基础系统拆包.md` 第三节第 4 条）：只认不上工具面的
        /// 工具（[`crate::tool::ToolRule::command_only`]），`miyu call` 调的（`false`）不认它们。
        command: bool,
    },
    /// 执行前的链判完了。
    Guarded {
        /// 到的时刻。
        at: Timestamp,
        /// 哪一个。
        id: NestedId,
        /// 链的结论。
        verdict: Verdict,
    },
    /// 跑完了。
    Done {
        /// 到的时刻。
        at: Timestamp,
        /// 哪一个。
        id: NestedId,
        /// 工具执行了，但是出了错。
        error: bool,
        /// 执行用了多少毫秒。
        duration_ms: Option<u64>,
        /// 交给脚本的那份结果存成的 blob；没存下来的没有。
        result: Option<ContentHash>,
        /// 那份结果有多少字节。
        bytes: Option<u64>,
    },
}

/// 交给执行器的嵌套调用的四种动作（[`crate::session::Action::Nested`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NestedAction {
    /// 过执行前的链，送回 [`NestedInput::Guarded`]。同 [`crate::session::Action::GuardTool`]。
    Guard {
        /// 哪一个。
        id: NestedId,
        /// 工具名。
        name: String,
        /// 修正过的参数。
        args: String,
        /// 工作目录：前台的照这一轮的，后台的照会话现在的环境。
        cwd: String,
        /// 加进来的目录，同上。
        dirs: Vec<String>,
        /// 实际生效的那一级。
        permission: Permission,
        /// 本会话放行过的规则。
        grants: Vec<RawJson>,
    },
    /// 跑，送回 [`NestedInput::Done`]。同 [`crate::session::Action::RunTool`]。
    Run {
        /// 哪一个。
        id: NestedId,
        /// 工具名。
        name: String,
        /// 修正过的参数。
        args: String,
        /// 工作目录。
        cwd: String,
        /// 加进来的目录。
        dirs: Vec<String>,
        /// 派出去那一刻实际生效的那一级：执行器照它写沙盒的规格。
        permission: Permission,
    },
    /// 掐掉在跑的那一个，不送回；之后到的不理。
    Cancel {
        /// 哪一个。
        id: NestedId,
    },
    /// 答脚本那头的 `miyu call`。
    Answer {
        /// 哪一个。
        id: NestedId,
        /// 怎么答。
        answer: NestedAnswer,
    },
}

/// 怎么答脚本那头（设计 39 第六节第 4 条的退出码由执行器照它定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NestedAnswer {
    /// 了结了，记过的那一条落了盘：了结成什么样，拒绝的那一句。
    Settled {
        /// 了结成什么样。
        status: NestedStatus,
        /// `denied` 的那一句。
        reason: Option<String>,
    },
    /// 没收下，什么都没记。
    Refused(NestedRefusal),
}

/// 没收下的几种：前一种是父级没了，后三种是用法不对（执行器给脚本退出码 2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestedRefusal {
    /// 父级不在了：前台的调用有了结果、在停，后台的任务报过结束了。
    Gone,
    /// 工具面上没有这个名字。
    Unknown,
    /// 不准脚本里调：访问类别不是 `read`、`network`、`execute`（设计 39 MC2），或者这件工具的 `clicall` 没开（施工 M-2）。
    NotAllowed,
    /// 参数修正不了。
    NotAnObject,
}

/// 一个在跑的嵌套调用（施工 M-5 上）：订阅时交给头的那一份，头重连以后照它接着画、接着计时。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NestedRunning {
    /// 嵌套调用编号。
    pub id: NestedId,
    /// 调的哪件工具。
    pub name: String,
    /// 修正过的参数，JSON 原文。
    pub args: String,
    /// 派去跑的时刻。
    pub at: Timestamp,
}

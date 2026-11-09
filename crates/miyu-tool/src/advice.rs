//! 问顾问的形状（设计 `69-旗舰顾问.md` 第三节，施工 ADV-1、ADV-2，`docs/blueprint/tools/advisor.md`）：`advisor` 这件工具经
//! 派子代理的端口（[`crate::AgentPort::advise`]）问旗舰池里的模型。端口同步等顾问答完，交回回答原文和顾问会话的编号。
//!
//! 和派子代理不同：派子代理是派活，当场交回编号、它在后台跑；问顾问是问主意，等它这一轮答完才交回（设计 69 第二节「同步、
//! 不后台」）。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::SessionId;

/// 问顾问的那件工具的名字（设计 69 第三节第 1 条）。
pub const ADVISOR: &str = "advisor";

/// 问到了：顾问这一轮的回答原文，和顾问会话的编号（头照它点进去看问答，设计 69 施工 ADV-3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advised {
    /// 顾问会话。
    pub session: SessionId,
    /// 回答原文：这一轮里她说的正文，接起来。
    pub answer: String,
}

/// 没问到。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotAdvised {
    /// 旗舰池里没有可用的模型（设计 69 第一节第 2 条）：没造会话、没发请求。
    NoModel,
    /// 顾问没答成：造不成会话、问不进去、请求出错、被打断。英文的一句原因，照原样交给她（设计 69 第三节第 3 条）。
    Failed(String),
}

impl fmt::Display for NotAdvised {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotAdvised::NoModel => f.write_str("no model in the flagship pool"),
            NotAdvised::Failed(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for NotAdvised {}

/// 问顾问的 future。
pub type Advising<'a> = Pin<Box<dyn Future<Output = Result<Advised, NotAdvised>> + Send + 'a>>;

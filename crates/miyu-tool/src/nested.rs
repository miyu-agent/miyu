//! 脚本调工具的端口（设计 39，原型 M-0）：`shell` 起的命令经 `miyu call` 发来的调用，交给它跑，拿回给脚本的结局。
//!
//! 原型只为实测省不省：执行器旁路判、旁路跑，不进内核、不记事件（设计 39 第四节是正式的做法）。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// 一次脚本调用的结局：`miyu call` 照它退出、印标准输出和标准错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// 退出码：0 成功，1 工具出错或权限拒绝，2 用法不对，3 被打断，4 要人确认（设计 39 第六节第 4 条）。
    pub code: u8,
    /// 印到标准输出的。
    pub stdout: String,
    /// 印到标准错误的。
    pub stderr: String,
}

impl Outcome {
    /// 没执行的：退出码 `code`，标准错误写 `why`。
    pub fn refused(code: u8, why: impl Into<String>) -> Outcome {
        let mut stderr = why.into();
        stderr.push('\n');
        Outcome {
            code,
            stdout: String::new(),
            stderr,
        }
    }
}

/// 端口交回的：跑完了的结局。
pub type Calling = Pin<Box<dyn Future<Output = Outcome> + Send + 'static>>;

/// 脚本调工具的端口：执行器照这一次 `shell` 调用造一个，只交给 `shell`。
pub trait NestedPort: Send + Sync {
    /// 调 `tool`，参数是 `args`（一个 JSON 对象的原文，值可以都是字符串，照工具的格式修）。
    fn call(&self, tool: String, args: String) -> Calling;

    /// `tool` 的完整说明和参数格式；不能调的是 `None`。
    fn schema(&self, tool: &str) -> Option<String>;
}

impl fmt::Debug for dyn NestedPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NestedPort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn NestedPort {
    fn eq(&self, other: &dyn NestedPort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn NestedPort {}

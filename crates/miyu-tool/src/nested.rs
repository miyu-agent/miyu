//! 脚本调工具的端口（施工 M-2，设计 `39-脚本调工具.md` 第四节、第六节）：她在 `shell` 里跑的命令经 `miyu call` 调这个会话的
//! 工具，一次调用就是挂在那次 `shell`（或者它放到后台的任务）下面的一次嵌套调用。执行器照每一次 `shell` 调用造一个，交给
//! [`crate::Call::nested`]；`shell` 把它交给通道的那一头（施工 M-3），通道每收一个请求调一次 [`NestedPort::call`]。
//!
//! 端口由下层定义、上层装（`00-设计理念.md` 第四节）：这里只定义形状，内核照直接调用的同一条链查、判、派，记过、落了盘
//! 才答回来。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::event::NestedStatus;
use miyu_kernel::session::NestedRefusal;

/// 调这个会话的工具：一次一件，等它了结。
pub trait NestedPort: Send + Sync {
    /// 调工具 `name`，参数是 `args`（一个 JSON 对象的原文，内核照工具的格式修）。父级结束、核心停了还没答的，交回
    /// [`Nested::Settled`] 的 `cancelled`。
    fn call(&self, name: String, args: String) -> Calling<'_>;
}

/// 等它了结的 future。
pub type Calling<'a> = Pin<Box<dyn Future<Output = Nested> + Send + 'a>>;

/// 一次嵌套调用的结局：脚本那头照它定退出码、印什么（设计 39 第六节第 3、4 条，施工 M-3）。
#[derive(Debug, Clone, PartialEq)]
pub enum Nested {
    /// 收下了，了结了，记过的那一条落了盘。
    Settled(Settled),
    /// 没收下，什么都没记：父级不在了，或者用法不对。
    Refused(NestedRefusal),
}

/// 了结了的那一次。
#[derive(Debug, Clone, PartialEq)]
pub struct Settled {
    /// 了结成什么样：跑完的 `ok`、`error`，链拦下的 `denied`，要问人的 `needs_approval`，没跑完的 `cancelled`。
    pub status: NestedStatus,
    /// `denied` 的那一句，照直接调用的原话。
    pub reason: Option<String>,
    /// 跑完的：工具交回的字，几段连起来。别的是空的。
    pub text: String,
    /// 跑完的：给脚本的结构化结果（[`crate::Done::data`]），有的才有。
    pub data: Option<serde_json::Value>,
    /// 跑完的：工具交回了图，没交给脚本。
    pub images_dropped: bool,
}

impl Settled {
    /// 没跑出结果的那几种：`denied`（带拒绝的那一句）、`needs_approval`、`cancelled`。
    pub fn without_result(status: NestedStatus, reason: Option<String>) -> Settled {
        Settled {
            status,
            reason,
            text: String::new(),
            data: None,
            images_dropped: false,
        }
    }
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

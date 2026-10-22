//! 脚本里调的工具（施工 M-5 上，设计 39 第七节第 4 条）：前台那次 `shell`、后台任务下面的嵌套调用。条目和会话状态里只带
//! 个数和最近一个，头照它在那一行后面接 `· #序号 显示名`；整份列表点开时照 `view.detail` 要。

use serde::Serialize;

use miyu_kernel::time::Timestamp;

use super::Title;

/// 一次 `shell`、一个任务下面调过的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Nested {
    /// 到这一刻有几个：了结的加在跑的。
    pub count: u32,
    /// 最近的那一个：序号最大的。
    pub last: NestedCall,
}

/// 一次嵌套调用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NestedCall {
    /// 序号：编号 `#` 后面那个数。
    pub n: u32,
    /// 工具名。
    pub name: String,
    /// 显示名和对象，照连接的语言（同工具那一步的标题）。
    pub title: Title,
    /// `running`，或者了结的 `ok`、`error`、`denied`、`needs_approval`、`cancelled`。
    pub state: String,
    /// 在跑的：派去跑的时刻，头照它写走了几秒。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<Timestamp>,
}

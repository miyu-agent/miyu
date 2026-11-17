//! 程序那几张表读成的样子（施工 PL-2 从 `package.rs` 挪出来：那边到了 500 行）：给 `miyu` 加的子命令 `[[command]]`、核心拉起
//! 的扩展 `[process]`、界面 `[ui]`、`miyu check` 跑的 `[check]`。读法在 `reader.rs`、`command.rs`。

use super::Capability;
use crate::phrases::Phrases;

/// `[command]`，`[[command]]` 的一个（施工 F-13）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// `miyu` 后面敲的那个词。
    pub name: String,
    /// 程序名，不带路径。
    pub program: String,
    /// 转交时加在人敲的参数前面的（施工 F-13）：没写的是空的。
    pub args: Vec<String>,
    /// 一句说明，`miyu -h` 照它列。
    pub about: Phrases,
    /// `name` 写在第几行：两个包撞了名，报在这一行。
    pub line: Option<usize>,
}

/// `[process]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    /// 拉起时带的参数。
    pub args: Vec<String>,
    /// 什么时候拉起。
    pub start: Start,
    /// 要哪些扩展能力（施工 9-4 下上）：照 [`Capability::ALL`] 的先后，不重复；没写的是空的。
    pub capabilities: Vec<Capability>,
}

/// 什么时候拉起。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// 开关打开才拉起（默认）。
    Manual,
    /// 核心起来就拉起。
    Always,
}

impl Start {
    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Start::Manual => "manual",
            Start::Always => "always",
        }
    }
}

/// `[ui]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pages {
    /// 认的页（`--page`）。
    pub opens: Vec<String>,
    /// 页面文件的目录，相对包文件夹（施工 G-15：包能装进数据根，原来相对资源目录的说法不成立）；没有的没有。
    pub pages_dir: Option<String>,
}

/// `[check]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// 跑 `<program> <args…>`。
    pub args: Vec<String>,
}

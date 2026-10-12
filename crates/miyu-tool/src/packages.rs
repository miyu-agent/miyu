//! 看软件包的端口（施工 F-10 上，设计 `31-软件包.md` 第六节，`docs/blueprint/tools/packages.md`）：`packages` 这件工具只拿它，
//! 不认识软件包清单和核心。协议端点造会话、载入时交进来，执行器交给每一次调用（[`crate::Call::packages`]）。
//!
//! 交回的是协议上那几样回应的样子（`package.list` 的一项、`package.info`、`package.install` 带 `preview` 的），名字、说明、
//! 写错的那一句照英文：工具照它写给她看的字。端口由下层定义、上层装（`00-设计理念.md` 第四节「依赖与接口的规矩」）。测试里
//! 的假调用没有，`packages` 说现在看不了。

use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use serde_json::Value;

/// 看软件包的那件工具的名字。
pub const PACKAGES: &str = "packages";

/// 装、卸软件包的那件工具的名字（施工 F-10 下，`docs/blueprint/tools/package.md`）。
pub const PACKAGE: &str = "package";

/// 看装了的软件包、看一个包文件夹；装、卸（施工 F-10 下）。
pub trait PackagesPort: Send + Sync {
    /// 装了的和卸掉了的出厂包：同 `package.list` 的 `packages`。
    fn list(&self) -> Looking<'_>;

    /// 包 `package`：同 `package.info`，多列表里那一项的 `name`、`summary`、`kind`。没装的交回 `unknown_package`。
    fn info(&self, package: String) -> Looking<'_>;

    /// 包文件夹 `path`（绝对路径）装上会是什么样：同 `package.install {path, preview: true}`。装不上的照真装那样拒（写错的
    /// 带上哪里不对、第几行）。
    fn inspect(&self, path: PathBuf) -> Looking<'_>;

    /// 装包文件夹 `path`（绝对路径，施工 F-10 下）：同 `package.install {path}`，交回装上的那一项（同 `package.list` 的一项）。
    /// 写错的、撞了的照协议拒。会话用的预设 `preset`（施工 F-10 下下）不开没列出来的软件、又没写这个包的，在那一份预设里
    /// 打开它（16 第三节）。
    fn install(&self, path: PathBuf, preset: Option<String>) -> Looking<'_>;

    /// 打开之前看一眼（施工 F-10 下下）：装着的包 `package` 带了什么、要什么能力，同装之前看一眼的样子。
    fn enabling(&self, package: String) -> Looking<'_>;

    /// 打开装着的包 `package`（施工 F-10 下下）：同 `package.enable`，它要的能力一并批（问人的那一次就是批）。
    fn enable(&self, package: String) -> Looking<'_>;

    /// 卸之前看一眼（施工 F-10 下）：同 `package.remove {package, preview: true}`。
    fn removing(&self, package: String) -> Looking<'_>;

    /// 卸包 `package`（施工 F-10 下）：同 `package.remove {package}`。
    fn remove(&self, package: String) -> Looking<'_>;
}

/// 端口的 future：成了交回那一份，不成交回为什么。
pub type Looking<'a> = Pin<Box<dyn Future<Output = Result<Value, PackageRefusal>> + Send + 'a>>;

/// 核心拒了：协议上的原因代码（`unknown_package`、`package_invalid`、`path_unreadable`……，核心正在停的是
/// `shutting_down`），写错的清单另带英文的一句哪里不对、第几行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRefusal {
    /// 原因代码。
    pub reason: String,
    /// 清单哪里不对：`package_invalid` 才有，英文。
    pub problem: Option<String>,
    /// 第几行：知道的才有。
    pub line: Option<u64>,
}

/// 端口不打出里面的东西。
impl fmt::Debug for dyn PackagesPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PackagesPort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn PackagesPort {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn PackagesPort {}

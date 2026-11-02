//! 交给 Miyu 拉起来的程序的环境变量（施工 FH-2下，设计 `55-数据根仿FHS.md` 第四节）：扩展、包带的命令、脚本拿到的几处目录叫
//! 什么名字、指到哪，只在这里定；拉起程序的几处（扩展、`miyu <包的命令>`、脚本工具）都照这里给。
//!
//! 每一种目录交的是一串名字（[`Names`]）：同一个位置同时用几个名字交出去。FH-3 起照设计 55 给新名字（`MIYU_STATE_DIR` 这些），
//! 旧名字过渡期间一起给、指到同一个位置，FH-6 删旧名字时只改这里。

use std::path::{Path, PathBuf};

use crate::root::DataRoot;

/// 同一个位置交出去的几个名字。
pub type Names = &'static [&'static str];

/// 数据根。
pub const HOME: Names = &["MIYU_HOME"];

/// 资源目录。
pub const RESOURCES: Names = &["MIYU_RESOURCES"];

/// 包的文件夹（施工 S-6 上）：出厂的在资源目录，装上的在数据根，由核心挑。
pub const APP: Names = &["MIYU_PACKAGE_DIR"];

/// 包自己的状态目录（施工 F-15）：扩展的工作目录也是它。
pub const PACKAGE_STATE: Names = &["MIYU_PACKAGE_STATE"];

/// 脚本的状态目录（施工 X-44）。
pub const SCRIPT_STATE: Names = &["MIYU_SCRIPT_STATE_DIR"];

/// 脚本的缓存目录（施工 Y-11 上）。
pub const SCRIPT_CACHE: Names = &["MIYU_SCRIPT_CACHE_DIR"];

/// 脚本交图的目录（施工 X-35）。
pub const SCRIPT_IMAGES: Names = &["MIYU_SCRIPT_IMAGES_DIR"];

/// 名字表 `names` 里每一个都指到 `value`。
pub fn named(names: Names, value: &Path) -> impl Iterator<Item = (&'static str, PathBuf)> + '_ {
    names.iter().map(move |name| (*name, value.to_path_buf()))
}

/// 一个包拉起时的几处：包的文件夹、状态目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDirs {
    /// 包的文件夹。
    pub app: PathBuf,
    /// 包自己的状态目录（[`DataRoot::package_state`]），不建：拉起的一方建好、换成真实位置再交。
    pub state: PathBuf,
}

impl PackageDirs {
    /// 数据根 `root` 里包 `id` 的几处，包的文件夹是 `app`。
    pub fn of(root: &DataRoot, id: &str, app: PathBuf) -> PackageDirs {
        PackageDirs {
            app,
            state: root.package_state(id),
        }
    }

    /// 交给它的环境变量：`state` 是拉起的一方建好、换成真实位置的状态目录。
    pub fn env(&self, state: &Path) -> Vec<(&'static str, PathBuf)> {
        named(APP, &self.app)
            .chain(named(PACKAGE_STATE, state))
            .collect()
    }
}

#[cfg(test)]
mod tests;

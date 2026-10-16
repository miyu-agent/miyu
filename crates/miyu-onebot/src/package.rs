//! 桥自己的包文件夹（施工 S-6 上，设计 `32-仓库拆分.md` 第九节第 2 条，作者说明第四节第 1 条）：清单 `package.toml` 在它
//! 根上，桥自己读的出厂数据、给模型看的字、给人看的字在它的 `data/` 里（原来在资源目录的 `software/onebot/`）。
//!
//! 在哪由核心定：核心拉起扩展、转交包的命令时把 `MIYU_PACKAGE_DIR` 设成这个包的文件夹（出厂的在资源目录、自己装的在家目录，
//! 哪一层就是哪一层）；桥不自己拼家目录的布局。没设的（老核心、人在终端里直接跑）退回资源目录的 `packages/onebot/`。

#[cfg(test)]
mod tests;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use miyu_client::places::ResourceRoot;

use crate::PACKAGE;

/// 核心交来包文件夹的环境变量。
pub const VARIABLE: &str = "MIYU_PACKAGE_DIR";

/// 包文件夹里放桥自己的数据的目录。
const DATA: &str = "data";

/// 资源目录里放出厂的软件包的目录：没设 [`VARIABLE`] 时照它找。
const SHIPPED: &str = "packages";

/// 这个包的文件夹。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    dir: PathBuf,
}

impl Package {
    /// 照环境变量 [`VARIABLE`] 的值 `given` 找：设了、不是空的就是它；没设、空的退回资源目录 `resources` 里出厂的那一份
    /// （`packages/onebot/`）。
    pub fn locate(given: Option<&OsStr>, resources: &ResourceRoot) -> Package {
        match given.filter(|given| !given.is_empty()) {
            Some(given) => Package::at(given),
            None => Package::at(resources.path().join(SHIPPED).join(PACKAGE)),
        }
    }

    /// 包文件夹就是 `dir`。
    pub fn at(dir: impl Into<PathBuf>) -> Package {
        Package { dir: dir.into() }
    }

    /// 包文件夹：清单 `package.toml` 在这里。
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 桥自己的数据在哪：包文件夹的 `data/`。
    pub fn data(&self) -> PathBuf {
        self.dir.join(DATA)
    }
}

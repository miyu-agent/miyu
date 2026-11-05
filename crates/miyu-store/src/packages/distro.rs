//! 系统加装那一层在哪（施工 SD-1，设计 `60-系统源分包.md` 第三节第 2 条）：系统的包管理器另外装的 Miyu 软件包（AUR、Homebrew
//! 发的终端、网页，别人经系统源发的扩展），一处一个 `<编号>/package.toml`，和出厂、装上的两层一样。
//!
//! - 设了 `MIYU_DISTRO_PACKAGES`（测试、演练用，要是绝对路径）的只看它，哪个平台都一样；
//! - Linux（别的类 Unix 照它）：`$XDG_DATA_DIRS` 里每一处的 `miyu/packages/`，没设或是空的照 `/usr/local/share:/usr/share`，先列的
//!   优先；不是绝对路径的那一段不算。Linuxbrew 自己的 `share` 由用户写进 `XDG_DATA_DIRS`；
//! - macOS：Homebrew 的 `<前缀>/share/miyu/packages/`，前缀是 `miyu` 所在目录的上一级，**不解开链接**（解开就进了 Cellar 里核心
//!   自己那一份）；
//! - Windows：没有这一层。
//!
//! 和出厂那一层是同一个目录的（真实路径一样）去掉：出厂的就是出厂的。同一处列了两次的只留第一次。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::env::Platform;

/// 测试、演练把这一层指到一处的环境变量。
pub const VARIABLE: &str = "MIYU_DISTRO_PACKAGES";

/// 没设 `XDG_DATA_DIRS` 时照 XDG 的默认。
const XDG_DEFAULT: &str = "/usr/local/share:/usr/share";

/// 找这一层要看的几样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
    /// 在哪个平台上。
    pub platform: Platform,
    /// `MIYU_DISTRO_PACKAGES`。
    pub only: Option<OsString>,
    /// `XDG_DATA_DIRS`。
    pub xdg_data_dirs: Option<OsString>,
    /// 程序被叫起来时的路径，不解开链接（macOS 上 `/opt/homebrew/bin/miyu` 这样）。
    pub called: Option<PathBuf>,
}

impl Where {
    /// 从进程里读一次。
    pub fn current() -> Where {
        Where {
            platform: Platform::current(),
            only: std::env::var_os(VARIABLE),
            xdg_data_dirs: std::env::var_os("XDG_DATA_DIRS"),
            called: std::env::current_exe().ok(),
        }
    }
}

/// 照 `place` 算系统加装那几处，照先后（先列的优先）；`shipped` 是出厂那一层的目录，和它一样的去掉。
pub fn dirs(place: &Where, shipped: &Path) -> Vec<PathBuf> {
    let wanted: Vec<PathBuf> = match place.only.as_ref().filter(|only| !only.is_empty()) {
        Some(only) => vec![PathBuf::from(only)],
        None => match place.platform {
            Platform::Windows => Vec::new(),
            Platform::Macos => place
                .called
                .as_deref()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .map(|prefix| vec![prefix.join("share").join("miyu").join("packages")])
                .unwrap_or_default(),
            Platform::Linux => {
                let listed = place
                    .xdg_data_dirs
                    .as_ref()
                    .filter(|dirs| !dirs.is_empty())
                    .cloned()
                    .unwrap_or_else(|| OsString::from(XDG_DEFAULT));
                // XDG 规定用 `:` 分开（不是 `split_paths` 那样随平台变）。
                listed
                    .to_string_lossy()
                    .split(':')
                    .filter(|dir| !dir.is_empty())
                    .map(|dir| Path::new(dir).join("miyu").join("packages"))
                    .collect()
            }
        },
    };
    let shipped = std::fs::canonicalize(shipped).unwrap_or_else(|_| shipped.to_path_buf());
    let mut dirs: Vec<PathBuf> = Vec::new();
    for dir in wanted {
        if !dir.is_absolute() || dirs.contains(&dir) {
            continue;
        }
        let real = std::fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
        if real == shipped {
            continue;
        }
        dirs.push(dir);
    }
    dirs
}

#[cfg(test)]
mod tests;

//! 系统的回收站（`10-自带软件.md` 第三节「`trash` 的细则」）：放进去（施工 4-6 下，`trash` 这件工具用）。施工 U-1 起撤销
//! 不再改回文件，移回来那一半拿掉了：进了回收站的，人自己从系统的回收站里找回。
//!
//! 三个平台各做一份（`trash/` 下的 `linux.rs`、`macos.rs`、`windows.rs`）：Linux 照 freedesktop 的回收站规范自己放，
//! macOS 用系统的 `trashItemAtURL`，Windows 用 `trash` 这个 crate、再读回收站里的 `$I` 记录找回它（`recycled.rs`）。
//! 记下的位置三个平台都是回收站里的真实路径，记进效果 `file.trashed`。

use std::io;
use std::path::Path;

#[cfg(target_os = "linux")]
#[path = "trash/linux.rs"]
mod bin;
#[cfg(target_os = "macos")]
#[path = "trash/macos.rs"]
mod bin;
#[cfg(windows)]
#[path = "trash/windows.rs"]
mod bin;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
#[path = "trash/other.rs"]
mod bin;
#[cfg_attr(
    not(windows),
    allow(
        dead_code,
        reason = "只有 Windows 用；每个平台都编它，它的测试到处都跑"
    )
)]
mod recycled;

/// 放不进回收站的几种。
#[derive(Debug)]
pub enum Refused {
    /// 这块盘上没有能放的回收站：没删。
    Unavailable,
    /// 挪了，可回收站里找不到它：它可能回不来了（只有 macOS、Windows 会碰到）。
    Lost,
    /// 出错了：系统说的原话。
    Failed(io::Error),
}

/// 把 `real` 放进回收站，交回它在回收站里的真实位置。`home` 是交给工具的家目录：Linux 的家目录回收站照它找。
///
/// # Errors
///
/// 这块盘上没有能放的回收站；挪了却找不到；系统报了错。
pub fn put(real: &Path, home: Option<&Path>) -> Result<String, Refused> {
    bin::put(real, home)
}

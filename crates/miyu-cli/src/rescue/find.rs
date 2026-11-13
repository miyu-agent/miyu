//! 找 `litemiyu`（施工 RC-1，`docs/blueprint/cli/rescue.md`「找程序」）：LiteMiyu 由系统的包管理器装，多半和 `miyu` 在同一个
//! 目录（AUR 的 `/usr/bin`、Homebrew 的 `bin/`）。照这个先后，取第一个是文件的：
//!
//! 1. `miyu` 被敲的那个路径（`argv[0]`）的同一个目录：不解链接。AUR 的 `/usr/bin/miyu` 链到 `/usr/lib/miyu/miyu`，`litemiyu`
//!    在 `/usr/bin`。只敲了名字的照 `PATH` 找到它被敲的那一个；相对路径照当前目录接。
//! 2. `miyu` 真实位置（`current_exe` 解了链接）的同一个目录：安装脚本、压缩包解开的放在一起。
//! 3. `PATH`。
//!
//! `PATH` 里相对的目录不认：那是照当前目录找，谁在当前目录放一个都能冒充（同 `miyu_store::packages::locate`）。Windows 上找
//! `litemiyu.exe`。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// 程序名，不带 `.exe`。
const PROGRAM: &str = "litemiyu";

/// 找的时候看的几样：都由调的一方取好，测试换成造的。
#[derive(Debug, Clone, Default)]
pub(crate) struct Places {
    /// `argv[0]`：被敲的那个路径，可能只是名字、相对路径、链接。
    pub(crate) typed: Option<PathBuf>,
    /// `current_exe`：这个程序在哪，可能还是链接。
    pub(crate) exe: Option<PathBuf>,
    /// `PATH` 的原值。
    pub(crate) path: Option<OsString>,
    /// 当前目录：`argv[0]` 是相对路径时照它接。
    pub(crate) cwd: Option<PathBuf>,
}

/// 照 [`Places`] 找 `litemiyu`，`windows` 是 Windows 上的找法（测试在别的平台上也走一遍）；都没有的是空的。
pub(crate) fn find(places: &Places, windows: bool) -> Option<PathBuf> {
    let file = named(PROGRAM, windows);
    let typed = places
        .typed
        .as_deref()
        .and_then(|typed| typed_dir(typed, places, windows));
    let real = places.exe.as_deref().and_then(|exe| {
        std::fs::canonicalize(exe)
            .unwrap_or_else(|_| exe.to_path_buf())
            .parent()
            .map(Path::to_path_buf)
    });
    typed
        .into_iter()
        .chain(real)
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file())
        .or_else(|| on_path(&file, places.path.as_ref()))
}

/// 被敲的那个路径所在的目录：带目录的照它（相对的接当前目录），只有名字的照 `PATH` 找。
fn typed_dir(typed: &Path, places: &Places, windows: bool) -> Option<PathBuf> {
    let has_dir = typed
        .parent()
        .is_some_and(|parent| !parent.as_os_str().is_empty());
    let path = match (has_dir, typed.is_absolute()) {
        (true, true) => typed.to_path_buf(),
        (true, false) => places.cwd.as_ref()?.join(typed),
        (false, _) => on_path(&named(typed.to_str()?, windows), places.path.as_ref())?,
    };
    path.parent().map(Path::to_path_buf)
}

/// `PATH` 里第一个有 `file` 这个文件的绝对目录里的它。
fn on_path(file: &str, path: Option<&OsString>) -> Option<PathBuf> {
    std::env::split_paths(path?)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(file))
        .find(|candidate| candidate.is_file())
}

/// 文件名：Windows 上没带 `.exe` 的接上。
fn named(name: &str, windows: bool) -> String {
    match windows && !name.to_ascii_lowercase().ends_with(".exe") {
        true => format!("{name}.exe"),
        false => name.to_string(),
    }
}

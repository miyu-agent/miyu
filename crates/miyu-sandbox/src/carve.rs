//! 藏起来的里面挖洞（施工 SW-6，Windows 用）：规格的约定是藏起来的里面，能写的（`write`）、单独放开只读的（`reveal`）照样
//! 透出来（[`crate::Spec::hides`]）。Linux 用挂载盖住再挂回来，macOS 深的规则压得过浅的，都照约定；Windows 的 MXC 交给系统
//! 的安全环境，禁止压过嵌在它里面的放开（2026-10-21、10-31 虚拟机上量过：默认工作区在数据根里写不进，Node 逐级 `lstat`
//! 到数据根里的 `packages` 被拒）。
//!
//! 所以交给 MXC 之前，一条藏起来的里面有要透出来的，就不禁这一条，改禁它下面不通往透出来的那几项，一层层往下，直到透出来的
//! 那一层。路上那几层落在整盘只读里：名字看得见，内容照旧读不到（10-31 虚拟机上对照过，数据根的令牌照旧读不到）。命令跑着时
//! 在路上那几层新建的不在禁止的里面。列不出一层的（没权限、不在），这一条照旧整条禁，宁可透不出来。
//!
//! 挖法是纯逻辑（[`carve`]，哪个系统都测）；列目录由调用的给。

use std::io;
use std::path::{Path, PathBuf};

/// `hidden` 里每一条，里面有 `open` 的（落在它里面、不是它本身）就换成它下面不通往 `open` 的那几项，一层层往下；没有的照原样。
/// `open` 本身、通往它的那几层都不进交回的；`open` 就是藏起来的那一条的，这一条不交（同 [`crate::Spec::hides`]：包住
/// 藏起来的那种放开压不过它，照旧禁）。先后照 `hidden`，
/// 换出来的照 `list` 列出来的先后。`list` 列一个目录下面的每一项（完整路径），列不出的这一条照原样整条交回。
pub fn carve(
    hidden: &[PathBuf],
    open: &[PathBuf],
    list: impl Fn(&Path) -> io::Result<Vec<PathBuf>>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in hidden {
        if open.contains(dir) {
            continue;
        }
        match below(dir, open, &list) {
            Some(denied) => out.extend(denied),
            None => out.push(dir.clone()),
        }
    }
    out
}

/// `dir` 下面要禁的：里面没有 `open` 的交回 `None`（整条禁）；列不出的也是 `None`。
fn below(
    dir: &Path,
    open: &[PathBuf],
    list: &impl Fn(&Path) -> io::Result<Vec<PathBuf>>,
) -> Option<Vec<PathBuf>> {
    if !open.iter().any(|open| open.starts_with(dir) && open != dir) {
        return None;
    }
    let entries = list(dir).ok()?;
    let mut denied = Vec::new();
    for entry in entries {
        if open.iter().any(|open| &entry == open) {
            continue;
        }
        match below(&entry, open, list) {
            Some(inner) => denied.extend(inner),
            None => denied.push(entry),
        }
    }
    Some(denied)
}

/// 照文件系统列 `dir` 下面的每一项。
///
/// # Errors
///
/// 列不出来：没权限、不在、不是目录。
pub fn entries(dir: &Path) -> io::Result<Vec<PathBuf>> {
    std::fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect()
}

#[cfg(test)]
mod tests;

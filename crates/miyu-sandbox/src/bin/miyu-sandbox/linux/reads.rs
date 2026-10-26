//! 读怎么放（`docs/blueprint/sandbox/linux.md`「怎么走」第 1 条第 3 步）：整盘能读，藏起来的除外。Landlock 只能放行、
//! 挖不了洞，所以从根目录一级级往下走：通向藏起来的那条路上的目录走进去，藏起来的本身跳过，别的整个放行。
//!
//! 写不照这个走法（施工 5-14 改，2026-10-22 项目主人定，照 dsh）：能写的整个放，不在里面挖洞；施工 5-15 到 5-14 改之间
//! 一级级绕开过，冻住了路上那一层。
//!
//! 链接跳过、不跟过去：跟过去就可能放行到藏起来的里面。读链接时照它指到的真实位置判，那里有没有放行照那里的规则。
//! 藏起来的路径先换成真实的位置：规格说好了给真实的位置，这里再换一次，免得一段链接让这条路对不上、藏不住。同一片文件另挂
//! 在别处的（挂载别名，施工 SEC-3），别名那一处也照藏起来的绕开（[`super::aliases`]）。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 藏起来的是 `hidden`：交回要整个放「读」的路径，每条连它下面。一条藏起来的都没有（或者都不在），就是根目录整个
/// 放。
///
/// # Errors
///
/// 通向藏起来的那条路上的目录读不了：说是哪一个、系统的原话。
pub(super) fn plan(hidden: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let real = super::aliases::with_aliases(&reals(hidden)?);
    let root = Path::new("/");
    if real.is_empty() {
        return Ok(vec![root.to_path_buf()]);
    }
    let mut out = Vec::new();
    walk(root, &real, &mut out)?;
    Ok(out)
}

/// 藏起来的几条换成真实的位置；不在的跳过。
fn reals(hidden: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut real = Vec::new();
    for path in hidden {
        match fs::canonicalize(path) {
            Ok(path) => real.push(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot open {}: {error}", path.display())),
        }
    }
    Ok(real)
}

/// `dir` 在通向藏起来的那条路上：它下面的每一样照规矩放进 `out`。
fn walk(dir: &Path, hidden: &[PathBuf], out: &mut Vec<PathBuf>) -> Result<(), String> {
    let unreadable = |error: io::Error| format!("cannot open {}: {error}", dir.display());
    for entry in fs::read_dir(dir).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let path = entry.path();
        if hidden.contains(&path) {
            continue;
        }
        let kind = entry.file_type().map_err(unreadable)?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() && hidden.iter().any(|secret| secret.starts_with(&path)) {
            walk(&path, hidden, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

//! 读怎么放（`docs/blueprint/sandbox/linux.md`「怎么走」第 1 条第 3 步）：整盘能读，藏起来的除外。Landlock 只能放行、
//! 挖不了洞，所以从根目录一级级往下走：通向藏起来的那条路上的目录走进去，藏起来的本身跳过，别的整个放行。
//!
//! 写不照这个走法（施工 5-14 改，2026-10-22 项目主人定，照 dsh）：能写的整个放，不在里面挖洞；施工 5-15 到 5-14 改之间
//! 一级级绕开过，冻住了路上那一层。
//!
//! 链接跳过、不跟过去：跟过去就可能放行到藏起来的里面。读链接时照它指到的真实位置判，那里有没有放行照那里的规则。
//! 藏起来的路径先换成真实的位置：规格说好了给真实的位置，这里再换一次，免得一段链接让这条路对不上、藏不住。同一片文件另挂
//! 在别处的（挂载别名，施工 SEC-3），别名那一处也照藏起来的绕开（[`super::aliases`]）。

use std::collections::BTreeMap;
use std::ffi::OsStr;
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
    plan_seen(hidden, &mut ())
}

/// 同 [`plan`]，一路看到的都告诉 `seen`（施工 V-47：核心缓存规则集时照它记指纹）：读到的挂载表、换成真实位置又加上别名的
/// 藏起来的、走进的每一个目录和它里面每一样怎么放的。
pub(super) fn plan_seen<S: Seen>(hidden: &[PathBuf], seen: &mut S) -> Result<Vec<PathBuf>, String> {
    let real = reals(hidden)?;
    let table = super::aliases::table();
    seen.table(table.as_deref());
    let real = super::aliases::with_table(&real, table.as_deref());
    seen.real(&real);
    let root = Path::new("/");
    if real.is_empty() {
        return Ok(vec![root.to_path_buf()]);
    }
    below_seen(root, &real, seen)
}

/// 一级级走时目录里的一样怎么放的（施工 V-47）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// 藏起来的，跳过。
    Hidden,
    /// 链接，跳过。
    Link,
    /// 通向藏起来的那条路上的目录，走进去。
    Down,
    /// 整个放读。
    Allow,
}

/// 一级级走时看到的（施工 V-47）：[`plan`] 不看（`()`），核心缓存规则集时照它记指纹（`print.rs`）。
pub(super) trait Seen {
    /// 读到的挂载表（读不了的没有）。
    fn table(&mut self, _table: Option<&[u8]>) {}
    /// 换成真实位置、加上别名以后的藏起来的。
    fn real(&mut self, _real: &[PathBuf]) {}
    /// 列出了目录 `dir`，接着是它里面的每一样。
    fn enter(&mut self, _dir: &Path) {}
    /// 目录里的一样（名字）怎么放的。
    fn entry(&mut self, _name: &OsStr, _step: Step) {}
    /// 这个目录看完了，回到上一级。
    fn leave(&mut self) {}
}

impl Seen for () {}

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

/// 藏起来的几条排成的树（施工 V-46）：一层是路径的一段（[`Path::components`]，和 [`Path`] 比相等、比开头一样照段比：
/// `//`、尾斜杠、中间的 `.` 不算一段），段照原样的字节比。键是这一段的字（`/`、`.`、`..`、名字）：一段名字里不会有
/// `/`，`.`、`..` 也不会是名字，互相认不混。走到一个目录时手里拿着它那一处，下面每一样照名字查一次：原来每一样都和
/// 每一条藏起来的整条比两遍，是 O(目录项 × 藏起来的条数)。
#[derive(Default)]
struct Node<'a> {
    /// 有一条藏起来的正好到这里。
    hidden: bool,
    /// 有藏起来的从这里接着往下的，照下一段分。
    below: BTreeMap<&'a OsStr, Node<'a>>,
}

impl<'a> Node<'a> {
    fn of(hidden: &'a [PathBuf]) -> Node<'a> {
        let mut tree = Node::default();
        for path in hidden {
            let mut at = &mut tree;
            for part in path.components() {
                at = at.below.entry(part.as_os_str()).or_default();
            }
            at.hidden = true;
        }
        tree
    }

    /// `path` 那一处；没有藏起来的经过它就是 `None`。
    fn at(&self, path: &Path) -> Option<&Node<'a>> {
        path.components()
            .try_fold(self, |at, part| at.below.get(part.as_os_str()))
    }
}

/// `dir` 在通向藏起来的那条路上：交回它下面照规矩放的。
#[cfg(test)]
fn below(dir: &Path, hidden: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    below_seen(dir, hidden, &mut ())
}

/// `dir` 在通向藏起来的那条路上：交回它下面照规矩放的，一路看到的告诉 `seen`。
fn below_seen<S: Seen>(
    dir: &Path,
    hidden: &[PathBuf],
    seen: &mut S,
) -> Result<Vec<PathBuf>, String> {
    let tree = Node::of(hidden);
    let mut out = Vec::new();
    walk(
        dir,
        tree.at(dir).unwrap_or(&Node::default()),
        &mut out,
        seen,
    )?;
    Ok(out)
}

/// `dir` 在通向藏起来的那条路上，`node` 是树里它那一处：它下面的每一样照规矩放进 `out`，每一样怎么放的告诉 `seen`。
fn walk<S: Seen>(
    dir: &Path,
    node: &Node<'_>,
    out: &mut Vec<PathBuf>,
    seen: &mut S,
) -> Result<(), String> {
    let unreadable = |error: io::Error| format!("cannot open {}: {error}", dir.display());
    let entries = fs::read_dir(dir).map_err(unreadable)?;
    seen.enter(dir);
    for entry in entries {
        let entry = entry.map_err(unreadable)?;
        let name = entry.file_name();
        let next = node.below.get(name.as_os_str());
        if next.is_some_and(|next| next.hidden) {
            seen.entry(&name, Step::Hidden);
            continue;
        }
        let kind = entry.file_type().map_err(unreadable)?;
        if kind.is_symlink() {
            seen.entry(&name, Step::Link);
            continue;
        }
        let path = entry.path();
        match next {
            Some(next) if kind.is_dir() => {
                seen.entry(&name, Step::Down);
                walk(&path, next, out, seen)?;
            }
            _ => {
                seen.entry(&name, Step::Allow);
                out.push(path);
            }
        }
    }
    seen.leave();
    Ok(())
}

#[cfg(test)]
mod tests;

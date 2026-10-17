//! 人格、预设共用的四层（`docs/blueprint/personas.md`、`presets.md`，`16-人格与预设.md` 第四节，设计 `31-软件包.md` 第十节）：出厂的、
//! 软件包带的（施工 P-11 上）、系统区的、管理员家目录里的，同名的后面的叠在前面的上面。这里只有层本身、包那一层认哪一份
//! （`covered`）和两样读盘的小事，各自怎么叠在 `personas`、`presets`。

use std::io;
use std::path::{Path, PathBuf};

/// 一层：从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// 随发行附带的，只读。
    Shipped,
    /// 装着的软件包带的（施工 P-11 上）：包文件夹里的 `personas/`、`presets/`，一个包一处，只读，卸包就没了。
    Package,
    /// 系统区，管理员给大家的。
    System,
    /// 管理员自己的家目录。
    Home,
}

impl Layer {
    /// 协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Shipped => "shipped",
            Layer::Package => "package",
            Layer::System => "system",
            Layer::Home => "home",
        }
    }
}

/// 几处里被盖住、不认的那几处（施工 P-11 上，设计 31 第十节第 7 条）：包那一层同一个编号只认一份。出厂的有这个编号的，包里的
/// 都不认（定了的 J）；几个包都有的，认排在前面的那个（交进来的先后：出厂的包先于家目录的，同一层照编号）。别的层不管。
/// `has` 说某一处有没有这个编号；交回 `(不认的那一处的下标, 盖住它的那一处的下标)`。
pub(crate) fn covered(
    dirs: &[(Layer, PathBuf)],
    has: impl Fn(&Path) -> bool,
) -> Vec<(usize, usize)> {
    let mut first: Option<usize> = None;
    let mut out = Vec::new();
    for (at, (layer, dir)) in dirs.iter().enumerate() {
        if !matches!(layer, Layer::Shipped | Layer::Package) || !has(dir) {
            continue;
        }
        match first {
            Some(by) => out.push((at, by)),
            None => first = Some(at),
        }
    }
    out
}

/// 同 [`covered`]，只要不认的那几处的下标。
pub(crate) fn hidden(dirs: &[(Layer, PathBuf)], has: impl Fn(&Path) -> bool) -> Vec<usize> {
    covered(dirs, has)
        .into_iter()
        .map(|(lost, _)| lost)
        .collect()
}

/// 包里带的一份被盖住了（施工 P-11 上，`miyu check` 报）：读的时候不认它。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Covered {
    /// 它在哪：人格的目录、预设的文件。
    pub path: PathBuf,
    /// 盖住它的那一份在哪：出厂的、排在前面的包里的。
    pub by: PathBuf,
}

/// 几层里所有编号 `ids` 被盖住的包里那几份（`covered`）。`at(dir, id)` 是某一处里这个编号的位置，`has` 照它在不在算。
pub(crate) fn covered_all(
    dirs: &[(Layer, PathBuf)],
    ids: &[String],
    at: impl Fn(&Path, &str) -> PathBuf,
    has: impl Fn(&Path) -> bool,
) -> Vec<Covered> {
    let mut out = Vec::new();
    for id in ids {
        for (lost, by) in covered(dirs, |dir| has(&at(dir, id))) {
            out.push(Covered {
                path: at(&dirs[lost].1, id),
                by: at(&dirs[by].1, id),
            });
        }
    }
    out
}

/// 几层的位置插上软件包那一层（施工 P-11 上）：每个包文件夹 `packages` 里的 `sub`（`personas`、`presets`），接在出厂的后面，
/// 照交进来的先后。
pub(crate) fn with_packages(dirs: &mut Vec<(Layer, PathBuf)>, packages: &[PathBuf], sub: &str) {
    let at = dirs
        .iter()
        .rposition(|(layer, _)| matches!(layer, Layer::Shipped | Layer::Package))
        .map_or(0, |at| at + 1);
    let added = packages.iter().map(|dir| (Layer::Package, dir.join(sub)));
    dirs.splice(at..at, added);
}

/// 真的位置；换不成的照原样。`miyu check` 写了文件时两边都照它比（施工 8-30）。
pub(crate) fn real(path: &Path) -> PathBuf {
    miyu_fs::resolve(Path::new("/"), None, &path.to_string_lossy())
        .unwrap_or_else(|_| path.to_path_buf())
}

/// 读一个文件：没有的是没有。
pub(crate) fn read_text(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// 人格、预设的编号合不合写法：小写字母开头，小写字母、数字、`-`、`_`，最多 64 个。它是一层目录或一个文件名，不许带路径。
pub fn valid(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && id.len() <= 64
}

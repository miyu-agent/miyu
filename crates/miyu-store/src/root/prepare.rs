//! 建骨架建不成的几种（从 `root.rs` 分出来，行数；代码是施工 LC-1）。

use std::fmt;
use std::io;
use std::path::PathBuf;

use super::MARKER;

/// 建骨架建不成。
#[derive(Debug)]
pub enum PrepareError {
    /// 目录里有别的东西，又没有标记：认不出是 Miyu 的数据根，一个字节都不动它。里面是什么不去猜。
    NotOurs(PathBuf),
    /// 有旧版的标记（[`OLD_MARKER`](super::OLD_MARKER)）、没有这一版的：旧版 Miyu 的数据根（布局 2），新版不认、不动（施工 FH-3，设计 55 第六节
    /// 第 1 条）。
    OldLayout(PathBuf),
    /// 标记写的不是这一版的布局（以后的版本、写坏了的）：不认、不动。
    OtherLayout(PathBuf),
    /// 读写出错。
    Io(io::Error),
}

impl PrepareError {
    /// 认得出是哪一种的代码（施工 LC-1）：核心起不来时写进那一行，头照 `start/<代码>` 取给人看的字（`{path}` 是数据根）。
    /// 读写出错的没有。
    pub fn code(&self) -> Option<&'static str> {
        match self {
            PrepareError::NotOurs(_) => Some("data_root_not_ours"),
            PrepareError::OldLayout(_) => Some("data_root_old_layout"),
            PrepareError::OtherLayout(_) => Some("data_root_other_layout"),
            PrepareError::Io(_) => None,
        }
    }
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrepareError::NotOurs(path) => write!(
                f,
                "{} 里有别的东西，认不出是 Miyu 的数据根（顶层没有 {MARKER}），不动它。设 MIYU_HOME 指到一个空目录",
                path.display()
            ),
            PrepareError::OldLayout(path) => write!(
                f,
                "{} 是旧版 Miyu 的数据根（layout 2），新版不认；设 MIYU_HOME 指到一个空目录，旧数据照施工单 FH-7 自己搬",
                path.display()
            ),
            PrepareError::OtherLayout(path) => write!(
                f,
                "{} 的 {MARKER} 写的不是这一版的布局（layout 3），不动它",
                path.display()
            ),
            PrepareError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for PrepareError {}

impl From<io::Error> for PrepareError {
    fn from(error: io::Error) -> PrepareError {
        PrepareError::Io(error)
    }
}

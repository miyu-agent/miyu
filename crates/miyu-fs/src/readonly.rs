//! 只禁写（施工 5-17 下，`docs/designs/11-权限与沙盒.md` 第四节「只禁写」，2026-10-22 项目主人定）：能写的范围里、改了以后会在
//! 沙盒外被执行的几样，读照旧，写要问人。出厂的清单，加上人在「隐私与安全」里加的、去掉的（同私密目录，记两份、不记整份）。
//!
//! 一项有三种写法：`~/…`（家目录下的那一处）、绝对路径、相对路径（每个工作区、加进来的目录里任一层的那一处，同原来写死的
//! `.git/hooks`：子仓库里的也算）。

use std::path::{Path, PathBuf};

use crate::{too_broad, written};

mod shipped;

pub use shipped::SHIPPED_READONLY;

/// 照这一轮的配置拼好的只禁写名单。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Readonly {
    /// 相对路径的：几段名字，工作区、加进来的目录里任一层接得上就算。
    pub inside: Vec<Vec<String>>,
    /// 家目录下的、绝对路径的：照原样拼好，还没换成真实的位置。
    pub paths: Vec<PathBuf>,
}

/// 人写的一项是哪一种。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadonlyEntry {
    /// 相对路径：几段名字。
    Inside(Vec<String>),
    /// `~`、`~/…`、绝对路径：拼好的路径。
    Path(PathBuf),
}

impl Readonly {
    /// 出厂的：家目录 `home` 不知道的，`~/…` 那几项不算。
    pub fn shipped(home: Option<&Path>) -> Readonly {
        Readonly::with(home, &[], &[])
    }

    /// 出厂的去掉 `remove` 里写了的（照原样的写法比），再加上 `add`。写法不对的、家目录本身和它的上级（[`too_broad`]）不算：手改
    /// 配置文件写了的，照不写算。家目录不知道的，`~` 开头的不算。重复的只留一个。
    pub fn with(home: Option<&Path>, add: &[String], remove: &[String]) -> Readonly {
        let mut list = Readonly::default();
        let shipped = SHIPPED_READONLY
            .iter()
            .filter(|shipped| !remove.iter().any(|removed| removed == *shipped))
            .map(|shipped| (*shipped).to_string());
        for text in shipped.chain(add.iter().cloned()) {
            match readonly_entry(home, &text) {
                Some(ReadonlyEntry::Inside(names)) if !list.inside.contains(&names) => {
                    list.inside.push(names);
                }
                Some(ReadonlyEntry::Path(path))
                    if !home.is_some_and(|home| too_broad(home, &path))
                        && !list.paths.contains(&path) =>
                {
                    list.paths.push(path);
                }
                _ => {}
            }
        }
        list
    }
}

/// 人写的一项（施工 5-17 下）：`~`、`~/…`、绝对路径照 [`written`] 拼（家目录不知道的没有）；别的当相对路径，照 `/` 拆成几段
/// （Windows 上 `\` 也算），空的、`.` 跳过，有 `..` 的、一段都没有的没有。
pub fn readonly_entry(home: Option<&Path>, text: &str) -> Option<ReadonlyEntry> {
    if text == "~" || text.starts_with("~/") || Path::new(text).is_absolute() {
        return home
            .or_else(|| Path::new(text).is_absolute().then_some(Path::new("/")))
            .and_then(|home| written(home, text))
            .map(ReadonlyEntry::Path);
    }
    let separators: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    let names: Vec<String> = text
        .split(separators)
        .filter(|name| !name.is_empty() && *name != ".")
        .map(str::to_string)
        .collect();
    (!names.is_empty() && !names.iter().any(|name| name == ".."))
        .then_some(ReadonlyEntry::Inside(names))
}

#[cfg(test)]
mod tests;

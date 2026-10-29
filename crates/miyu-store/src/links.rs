//! 找脚本、技能、斜杠命令模板时列一层目录（施工 X-37，`docs/blueprint/skills.md`「技能」第 1 条、「脚本」第 1 条，`slash.md`
//! 「在哪」）：认链接，照链接指到的东西认，指到文件的算文件、指到目录的算目录（2026-10-21 项目主人定，照别家 harness 的
//! 做法）；断了的、绕回自己的跳过，说为什么。
//!
//! 真实位置落在数据根里的，只认数据根里的 `scripts`、`skills`、`commands`、`packages` 下面（[`Fence`]）：她读得到
//! 技能、沙盒放开脚本都照真实的位置，不收窄的话一个链接就能把数据根下的本机令牌、密钥文件拉进来。落在别处的跳过，
//! 带上真实的位置（`miyu check` 照它说指到了哪一片）。不是链接、所在的目录是链接的同样照真实的位置查。

use std::path::{Path, PathBuf};

use crate::root::DataRoot;
use crate::skills::Skipped;

/// 一层里认了的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    /// 在这一层里的位置：是链接的就是链接本身。
    pub(crate) path: PathBuf,
    /// 名字。
    pub(crate) name: String,
    /// 是链接的：它最终指到的真实位置。不是的没有。
    pub(crate) real: Option<PathBuf>,
}

/// 找的是哪一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// 文件（脚本、模板）。
    File,
    /// 目录（技能文件夹）。
    Dir,
}

/// 数据根里认的地方：真实位置落在数据根里的，只认那几处下面。
#[derive(Debug, Clone)]
pub(crate) struct Fence {
    /// 数据根的真实位置；不碰数据根的（装之前看包带了什么）没有，哪里都认。
    root: Option<PathBuf>,
    /// 认的那几处的真实位置。
    own: Vec<PathBuf>,
}

impl Fence {
    /// 数据根 `root` 里自己放东西的几处（[`DataRoot::own_places`]），加装上的包那一处（施工 U23 上上，[`DataRoot::real_packages`]）。
    pub(crate) fn of(root: &DataRoot) -> Fence {
        let mut own = root.own_places();
        own.push(root.real_packages());
        Fence {
            own,
            root: Some(real(root.path())),
        }
    }

    /// 哪里都认：装之前看包文件夹带了什么，不碰数据根。
    pub(crate) fn anywhere() -> Fence {
        Fence {
            root: None,
            own: Vec::new(),
        }
    }

    /// 真实位置 `real` 认不认：数据根外面的认，里面的只认那几处下面。
    fn admits(&self, real: &Path) -> bool {
        self.root.as_ref().is_none_or(|root| {
            !real.starts_with(root) || self.own.iter().any(|dir| real.starts_with(dir))
        })
    }
}

/// `place` 下面名字合 `wanted`、种类是 `kind` 的项，照名字排；没有这个目录的是空的，点开头的不算。照链接指到的认种类，
/// 对不上的不算；断了的、绕回自己的，和真实位置不在 `fence` 认的地方的，另交回跳过（后者的 `outside` 是真实的位置），
/// 照位置排。
pub(crate) fn list(
    place: &Path,
    kind: Kind,
    fence: &Fence,
    wanted: impl Fn(&str) -> bool,
) -> (Vec<Entry>, Vec<Skipped>) {
    let Ok(entries) = std::fs::read_dir(place) else {
        return (Vec::new(), Vec::new());
    };
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') || !wanted(&name) {
            continue;
        }
        let path = entry.path();
        let linked = entry.file_type().is_ok_and(|kind| kind.is_symlink());
        // 照指到的看种类：断了的、绕回自己的看不了。
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                if linked {
                    skipped.push(Skipped::new(path, format!("broken link: {error}")));
                }
                continue;
            }
        };
        if metadata.is_dir() != (kind == Kind::Dir) {
            continue;
        }
        let Ok(real) = std::fs::canonicalize(&path) else {
            continue;
        };
        if !fence.admits(&real) {
            let why = format!(
                "links to {} inside the data root, outside the folders you keep things in",
                real.display()
            );
            skipped.push(Skipped {
                outside: Some(real),
                ..Skipped::new(path, why)
            });
            continue;
        }
        found.push(Entry {
            path,
            name,
            real: linked.then_some(real),
        });
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    skipped.sort_by(|a, b| a.path.cmp(&b.path));
    (found, skipped)
}

/// 真实的位置，换不成的照原样（还没建的目录）。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests;

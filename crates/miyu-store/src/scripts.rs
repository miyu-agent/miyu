//! 找脚本（施工 X-12 上，`docs/blueprint/skills.md`「脚本」，设计 `31-软件包.md` 第五节）：一个脚本是一个文件，开头的注释写
//! 说明、参数（`miyu_policy::script`）。从两处找：
//!
//! - 装了的软件包（清单读成了的）的 `scripts/<文件>`；
//! - 本机自己放的 `home/<账号>/scripts/<文件>`。
//!
//! 同一个工具名的自己放的赢；两个包撞了照包的编号留前一个。读不了的、不是字的、开头写错的跳过，交回给调的一方记。子文件夹、
//! 点开头的不算。碰磁盘，调的一方放在阻塞线程里。

use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;
use miyu_policy::script::{self, ScriptHead};

use crate::packages::Found;
use crate::root::DataRoot;
use crate::skills::Skipped;

/// 一个找到的脚本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// 开头读出来的。
    pub head: ScriptHead,
    /// 脚本文件。
    pub file: PathBuf,
    /// 哪个软件包带的；自己放的没有。
    pub package: Option<String>,
}

/// 找脚本：装了的软件包 `packages` 里的，加数据根 `root` 里账号 `account` 自己放的。照工具名排。
pub fn find(
    packages: &[Found],
    root: &DataRoot,
    account: &AccountId,
) -> (Vec<Script>, Vec<Skipped>) {
    let mut scripts: Vec<Script> = Vec::new();
    let mut skipped = Vec::new();
    let own = root.account_dir(account).join("scripts");
    let mut places: Vec<(PathBuf, Option<String>)> = vec![(own, None)];
    let mut installed: Vec<&Found> = packages.iter().filter(|found| found.read.is_ok()).collect();
    installed.sort_by(|a, b| a.id.cmp(&b.id));
    places.extend(
        installed
            .into_iter()
            .map(|found| (found.files_dir().join("scripts"), Some(found.id.clone()))),
    );
    for (place, package) in places {
        for (file, name) in files(&place) {
            let head = match std::fs::read(&file) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => script::read(&name, &text),
                    Err(_) => {
                        skipped.push(Skipped {
                            path: file,
                            why: "not text".to_string(),
                        });
                        continue;
                    }
                },
                Err(error) => {
                    skipped.push(Skipped {
                        path: file,
                        why: format!("not readable: {error}"),
                    });
                    continue;
                }
            };
            match head {
                Ok(head) if scripts.iter().any(|found| found.head.name == head.name) => {
                    skipped.push(Skipped {
                        path: file,
                        why: format!("script {:?} is already provided elsewhere", head.name),
                    });
                }
                Ok(head) => scripts.push(Script {
                    head,
                    file,
                    package: package.clone(),
                }),
                Err(problem) => skipped.push(Skipped {
                    path: file,
                    why: problem.to_string(),
                }),
            }
        }
    }
    scripts.sort_by(|a, b| a.head.name.cmp(&b.head.name));
    (scripts, skipped)
}

/// `place` 下面的文件和它的名字，照名字排；没有这个目录的是空的。子文件夹、点开头的不算。
fn files(place: &Path) -> Vec<(PathBuf, String)> {
    let Ok(entries) = std::fs::read_dir(place) else {
        return Vec::new();
    };
    let mut files: Vec<(PathBuf, String)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (!name.starts_with('.')).then(|| (entry.path(), name))
        })
        .collect();
    files.sort_by(|a, b| a.1.cmp(&b.1));
    files
}

#[cfg(test)]
mod tests;

//! 找技能（施工 X-10 上，`docs/blueprint/skills.md`「是什么」第 2 条，设计 `31-软件包.md` 第五节）：一个技能是一个文件夹，
//! 里面一份 `SKILL.md`（开头照公开的 Agent Skills 格式，`miyu_policy::skill`）。从两处找：
//!
//! - 装了的软件包（清单读成了的）的 `skills/<名字>/SKILL.md`，两层都算（出厂的在资源目录，自己装的在家目录）；
//! - 本机自己放的 `home/<账号>/skills/<名字>/SKILL.md`。
//!
//! 同名的自己放的赢；两个包撞了名字，照包的编号先后留前一个；后来的跳过、记下被谁盖住（施工 X-13 中）。读不了的、开头写错的
//! 跳过，交回给调的一方记（[`Skipped`]）。
//! 碰磁盘，调的一方放在阻塞线程里。

use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;
use miyu_policy::skill::{self, SkillHead};

use crate::packages::Found;
use crate::root::DataRoot;

/// 技能文件的名字。
const FILE: &str = "SKILL.md";

/// 一个找到的技能。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// 开头读出来的：名字、说明、给人看的两格。
    pub head: SkillHead,
    /// `SKILL.md` 在哪：她照它读。
    pub file: PathBuf,
    /// 技能的文件夹：开着的单独放开只读（`session/guard.md`）。
    pub dir: PathBuf,
    /// 哪个软件包带的；自己放的没有。
    pub package: Option<String>,
}

/// 跳过的一个：在哪、为什么（英文一句，运行日志、`miyu check` 用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// 那份 `SKILL.md`，或者撞了名的文件夹。
    pub path: PathBuf,
    /// 为什么。
    pub why: String,
    /// 同名被盖住的（施工 X-13 中，设计 `31-软件包.md` 第五节第 3 条）：盖住它的那一份（`SKILL.md`、脚本文件）。
    pub by: Option<PathBuf>,
}

/// 找技能：装了的软件包 `packages` 里的，加数据根 `root` 里账号 `account` 自己放的。照名字排。
pub fn find(
    packages: &[Found],
    root: &DataRoot,
    account: &AccountId,
) -> (Vec<Skill>, Vec<Skipped>) {
    let mut skills: Vec<Skill> = Vec::new();
    let mut skipped = Vec::new();
    let own = root.account_dir(account).join("skills");
    let mut places: Vec<(PathBuf, Option<String>)> = vec![(own, None)];
    let mut installed: Vec<&Found> = packages.iter().filter(|found| found.read.is_ok()).collect();
    installed.sort_by(|a, b| a.id.cmp(&b.id));
    places.extend(
        installed
            .into_iter()
            .map(|found| (found.files_dir().join("skills"), Some(found.id.clone()))),
    );
    for (place, package) in places {
        for (dir, name) in folders(&place) {
            let file = dir.join(FILE);
            let head = match head(&file, &name) {
                None => continue,
                Some(Ok(head)) => Ok(head),
                Some(Err(why)) => Err(why),
            };
            match head {
                Ok(head) => match skills.iter().find(|found| found.head.name == head.name) {
                    Some(winner) => skipped.push(Skipped {
                        path: dir,
                        why: format!(
                            "skill {:?} is already provided by {}",
                            head.name,
                            winner.file.display()
                        ),
                        by: Some(winner.file.clone()),
                    }),
                    None => skills.push(Skill {
                        head,
                        file,
                        dir,
                        package: package.clone(),
                    }),
                },
                Err(why) => skipped.push(Skipped {
                    path: file,
                    why,
                    by: None,
                }),
            }
        }
    }
    skills.sort_by(|a, b| a.head.name.cmp(&b.head.name));
    (skills, skipped)
}

/// 包文件夹 `dir` 带的技能（施工 X-13 上，装之前给人看）：`skills/` 下开头读得成的名字，照名字排；读不成的另交回为什么
/// （施工 X-13 补：不说的话她、人都看不出来少了一个）。
pub fn carried(dir: &Path) -> (Vec<String>, Vec<Skipped>) {
    let mut names = Vec::new();
    let mut skipped = Vec::new();
    for (folder, name) in folders(&dir.join("skills")) {
        let file = folder.join(FILE);
        match head(&file, &name) {
            None => {}
            Some(Ok(head)) => names.push(head.name),
            Some(Err(why)) => skipped.push(Skipped {
                path: file,
                why,
                by: None,
            }),
        }
    }
    (names, skipped)
}

/// 读文件夹名是 `name` 的那份 `SKILL.md`（`file`）的开头：没有这个文件的没有；读不了、写错了的交回为什么。
fn head(file: &Path, name: &str) -> Option<Result<SkillHead, String>> {
    match std::fs::read_to_string(file) {
        Ok(text) => Some(skill::read(name, &text).map_err(|problem| problem.to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(Err(format!("not readable: {error}"))),
    }
}

/// `place` 下面的子文件夹和它的名字，照名字排；没有这个目录的是空的。点开头的不算。
fn folders(place: &Path) -> Vec<(PathBuf, String)> {
    let Ok(entries) = std::fs::read_dir(place) else {
        return Vec::new();
    };
    let mut folders: Vec<(PathBuf, String)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (!name.starts_with('.')).then(|| (entry.path(), name))
        })
        .collect();
    folders.sort_by(|a, b| a.1.cmp(&b.1));
    folders
}

#[cfg(test)]
mod tests;

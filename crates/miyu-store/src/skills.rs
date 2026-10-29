//! 找技能（施工 X-10 上，`docs/blueprint/skills.md`「是什么」第 2 条，设计 `31-软件包.md` 第五节）：一个技能是一个文件夹，
//! 里面一份 `SKILL.md`（开头照公开的 Agent Skills 格式，`miyu_policy::skill`）。从三处找：
//!
//! - 装了的软件包（清单读成了的）的 `skills/<名字>/SKILL.md`，两层都算（出厂的在资源目录，自己装的在家目录）；
//! - 本机自己放的 `skills/<名字>/SKILL.md`；
//! - 配置开着时别的目录里的 `<名字>/SKILL.md`（施工 PT-2 中、PT-2 外，[`external`]：出厂一项 `~/.agents/skills`），照目录表的先后。
//!
//! 同名的自己放的赢，其次包里的，别的目录排最后、几个之间照表里的先后；别的目录里被压掉的不记（别的客户端的技能，不算 Miyu
//! 这边写错了）。两个包撞了名字，出厂的先于家目录、同一层照包的编号留前一个（装的时候撞了的装不上，施工 F-8 六补；
//! 撞得上的只剩核心升级后出厂的新带了同名的）；后来的跳过、记下被谁盖住（施工 X-13 中）。读不了的、开头写错的
//! 跳过，交回给调的一方记（[`Skipped`]）。技能文件夹可以是链接（施工 X-37，`links.rs`）：照它指到的真实位置记。
//! 碰磁盘，调的一方放在阻塞线程里。

use std::path::{Path, PathBuf};

use miyu_policy::skill::{self, SkillHead};

use crate::links::{self, Fence, Kind};
use crate::packages::Found;
use crate::root::DataRoot;

pub mod external;

pub use external::External;

/// 技能文件的名字。
const FILE: &str = "SKILL.md";

/// 一个找到的技能。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// 开头读出来的：名字、说明、给人看的两格。
    pub head: SkillHead,
    /// `SKILL.md` 在哪：她照它读。
    pub file: PathBuf,
    /// 技能的文件夹：开着的单独放开只读（`session/guard.md`）。是链接的照它指到的真实位置（施工 X-37）：沙盒只放开那一个
    /// 文件夹，她照真实的位置读得到。
    pub dir: PathBuf,
    /// 哪个软件包带的；自己放的、别的目录里的没有。
    pub package: Option<String>,
    /// 别的目录（施工 PT-2 中、PT-2 外，[`external`]）里的：配置里写的那个目录的原文。Miyu 不删它，别的客户端也在用。
    pub external: Option<String>,
    /// 技能文件夹是链接的（施工 X-37）：链接本身的位置（删的时候只删它，不碰指到的文件夹）。不是的没有。
    pub link: Option<PathBuf>,
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
    /// 链接指到数据根里不认的地方的（施工 X-37）：它的真实位置。
    pub outside: Option<PathBuf>,
}

impl Skipped {
    /// 在 `path`、为什么是 `why`，没被谁盖住、不是链接指出去的。
    pub(crate) fn new(path: PathBuf, why: String) -> Skipped {
        Skipped {
            path,
            why,
            by: None,
            outside: None,
        }
    }
}

/// 主人自己放技能的文件夹：`skills/`，还没建的也照写（施工 X-27 补，网页「打开目录」要）。
pub fn own_dir(root: &DataRoot) -> PathBuf {
    root.path().join("skills")
}

/// 找技能：数据根 `root` 里自己放的，加装了的软件包 `packages` 里的，加别的目录 `external`
/// 里的（施工 PT-2 外：配置关着的交空的，一个都不碰）。照名字排。
pub fn find(
    packages: &[Found],
    root: &DataRoot,
    external: &[External],
) -> (Vec<Skill>, Vec<Skipped>) {
    let mut skills: Vec<Skill> = Vec::new();
    let mut skipped = Vec::new();
    let own = own_dir(root);
    // 自己放的盖过包里的，包里的盖过别的目录里的：先到的赢。
    let mut places: Vec<(PathBuf, Option<String>, Option<&str>)> = vec![(own, None, None)];
    let mut installed: Vec<&Found> = packages.iter().filter(|found| found.read.is_ok()).collect();
    // 出厂的先于家目录，同一层照编号（施工 F-8 六补）：核心升级后出厂的新带了同名的，出厂的赢（设计 31 定了的 J）。
    installed.sort_by(|a, b| (a.layer, &a.id).cmp(&(b.layer, &b.id)));
    places.extend(installed.into_iter().map(|found| {
        (
            found.files_dir().join("skills"),
            Some(found.id.clone()),
            None,
        )
    }));
    // 别的目录排最后，照表里的先后（施工 PT-2 外）。
    places.extend(
        external
            .iter()
            .map(|one| (one.dir.clone(), None, Some(one.text.as_str()))),
    );
    let fence = Fence::of(root);
    for (place, package, from) in places {
        let (entries, unlisted) = links::list(&place, Kind::Dir, &fence, |_| true);
        skipped.extend(unlisted);
        for entry in entries {
            // 跳过的照它在 `skills/` 里的位置说，人认得出是哪一项；读、放开照真实的位置。
            let at = entry.path;
            let dir = entry.real.clone().unwrap_or_else(|| at.clone());
            let link = entry.real.is_some().then(|| at.clone());
            let file = dir.join(FILE);
            let head = match head(&file, &entry.name) {
                None => continue,
                Some(Ok(head)) => Ok(head),
                Some(Err(why)) => Err(why),
            };
            match head {
                Ok(head) => match skills.iter().find(|found| found.head.name == head.name) {
                    // 别的目录里被压掉的不记（施工 PT-2 外）。
                    Some(_) if from.is_some() => {}
                    Some(winner) => skipped.push(Skipped {
                        by: Some(winner.file.clone()),
                        ..Skipped::new(
                            at,
                            format!(
                                "skill {:?} is already provided by {}",
                                head.name,
                                winner.file.display()
                            ),
                        )
                    }),
                    None => skills.push(Skill {
                        head,
                        file,
                        dir,
                        package: package.clone(),
                        external: from.map(str::to_string),
                        link,
                    }),
                },
                Err(why) => skipped.push(Skipped::new(at.join(FILE), why)),
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
    let (entries, unlisted) =
        links::list(&dir.join("skills"), Kind::Dir, &Fence::anywhere(), |_| true);
    skipped.extend(unlisted);
    for entry in entries {
        let file = entry.path.join(FILE);
        match head(&file, &entry.name) {
            None => {}
            Some(Ok(head)) => names.push(head.name),
            Some(Err(why)) => skipped.push(Skipped::new(file, why)),
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

#[cfg(test)]
mod tests;

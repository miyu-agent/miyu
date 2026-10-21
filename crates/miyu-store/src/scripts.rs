//! 找脚本（施工 X-12 上，`docs/blueprint/skills.md`「脚本」，设计 `31-软件包.md` 第五节）：一个脚本是一个文件，开头的注释写
//! 说明、参数（`miyu_policy::script`）。从两处找：
//!
//! - 装了的软件包（清单读成了的）的 `scripts/<文件>`；
//! - 本机自己放的 `home/<账号>/scripts/<文件>`。
//!
//! 同一个工具名的自己放的赢；两个包撞了，出厂的先于家目录、同一层照包的编号留前一个（装的时候撞了的装不上，施工 F-8 六补），
//! 后来的跳过、记下被谁盖住（施工 X-13 中）。读不了的、不是字的、
//! 开头写错的跳过，交回给调的一方记。子文件夹、
//! 点开头的不算。脚本可以是链接（施工 X-37，`links.rs`）：照它指到的认，[`Script::file`] 记链接本身（放行出沙盒时
//! 照它看是不是链接）。碰磁盘，调的一方放在阻塞线程里。

use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;
use miyu_policy::script::{self, ScriptHead};

use crate::links::{self, Fence, Kind};
use crate::packages::Found;
use crate::root::DataRoot;
use crate::skills::Skipped;

/// 家目录里自己放的脚本登记工具时挂的包编号（施工 X-12 中）：包编号的写法里没有 `~`，撞不上真的包。
pub const HOME_PACKAGE: &str = "~home";

/// 一个找到的脚本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// 开头读出来的。
    pub head: ScriptHead,
    /// 脚本文件：是链接的就是链接本身（施工 X-37：执行器照它看是不是链接，起的时候照它指到的）。
    pub file: PathBuf,
    /// 哪个软件包带的；自己放的没有。
    pub package: Option<String>,
}

/// 账号 `account` 自己放脚本的文件夹：`home/<账号>/scripts/`，还没建的也照写（施工 X-27 补，网页「打开目录」要）。
pub fn own_dir(root: &DataRoot, account: &AccountId) -> PathBuf {
    root.account_dir(account).join("scripts")
}

/// 找脚本：装了的软件包 `packages` 里的，加数据根 `root` 里账号 `account` 自己放的。照工具名排。
pub fn find(
    packages: &[Found],
    root: &DataRoot,
    account: &AccountId,
) -> (Vec<Script>, Vec<Skipped>) {
    let mut scripts: Vec<Script> = Vec::new();
    let mut skipped = Vec::new();
    let own = own_dir(root, account);
    let mut places: Vec<(PathBuf, Option<String>)> = vec![(own, None)];
    let mut installed: Vec<&Found> = packages.iter().filter(|found| found.read.is_ok()).collect();
    // 出厂的先于家目录，同一层照编号（施工 F-8 六补）：核心升级后出厂的新带了同名的，出厂的赢（设计 31 定了的 J）。
    installed.sort_by(|a, b| (a.layer, &a.id).cmp(&(b.layer, &b.id)));
    places.extend(
        installed
            .into_iter()
            .map(|found| (found.files_dir().join("scripts"), Some(found.id.clone()))),
    );
    let fence = Fence::of(root, account);
    for (place, package) in places {
        let (entries, unlisted) = links::list(&place, Kind::File, &fence, |_| true);
        skipped.extend(unlisted);
        for entry in entries {
            let file = entry.path;
            match head(&file, &entry.name) {
                Ok(head) => match scripts.iter().find(|found| found.head.name == head.name) {
                    Some(winner) => skipped.push(Skipped {
                        by: Some(winner.file.clone()),
                        ..Skipped::new(
                            file,
                            format!(
                                "script {:?} is already provided by {}",
                                head.name,
                                winner.file.display()
                            ),
                        )
                    }),
                    None => scripts.push(Script {
                        head,
                        file,
                        package: package.clone(),
                    }),
                },
                Err(why) => skipped.push(Skipped::new(file, why)),
            }
        }
    }
    scripts.sort_by(|a, b| a.head.name.cmp(&b.head.name));
    (scripts, skipped)
}

/// 包文件夹 `dir` 带的脚本（施工 X-13 上，装之前给人看）：`scripts/` 下开头读得成的工具名，照文件名排；读不成的另交回为什么
/// （施工 X-13 补）。
pub fn carried(dir: &Path) -> (Vec<String>, Vec<Skipped>) {
    let mut names = Vec::new();
    let mut skipped = Vec::new();
    let (entries, unlisted) =
        links::list(&dir.join("scripts"), Kind::File, &Fence::anywhere(), |_| {
            true
        });
    skipped.extend(unlisted);
    for entry in entries {
        match head(&entry.path, &entry.name) {
            Ok(head) => names.push(head.name),
            Err(why) => skipped.push(Skipped::new(entry.path, why)),
        }
    }
    (names, skipped)
}

/// 读文件名是 `name` 的脚本 `file` 的开头：读不了、不是字、写错了的交回为什么。
fn head(file: &Path, name: &str) -> Result<ScriptHead, String> {
    let bytes = std::fs::read(file).map_err(|error| format!("not readable: {error}"))?;
    let text = String::from_utf8(bytes).map_err(|_| "not text".to_string())?;
    script::read(name, &text).map_err(|problem| problem.to_string())
}

#[cfg(test)]
mod tests;

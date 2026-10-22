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

/// 脚本的状态目录都在数据根 `data_root` 的这下面（施工 X-44）：`state/scripts/`。卸包、`miyu check` 照它找。
pub fn state_root(data_root: &Path) -> PathBuf {
    data_root.join("state").join("scripts")
}

/// 账号 `account` 跑来处是 `source`（包的编号，家目录里放的是 [`HOME_PACKAGE`]）、工具名是 `tool` 的脚本时给它的状态目录
/// （施工 X-44，`MIYU_SCRIPT_STATE_DIR`）：`state/scripts/<账号>/<来处>/<工具名>/`。不建：跑之前脚本工具建。
pub fn state_dir(data_root: &Path, account: &AccountId, source: &str, tool: &str) -> PathBuf {
    state_root(data_root)
        .join(account.as_str())
        .join(source)
        .join(tool)
}

/// 一格状态目录（施工 X-44）：`state/scripts/<账号>/<来处>/<工具名>/`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// 来处：包的编号，或者 [`HOME_PACKAGE`]。
    pub source: String,
    /// 工具名。
    pub tool: String,
    /// 目录。
    pub path: PathBuf,
}

/// 数据根 `data_root` 里所有账号的状态目录，照路径排（施工 X-44：`miyu check` 照它报没有脚本的那几格）。读不了的那一层跳过。
pub fn states(data_root: &Path) -> Vec<State> {
    let mut found = Vec::new();
    for account in folders(&state_root(data_root)) {
        for from in folders(&account) {
            for path in folders(&from) {
                if let (Some(source), Some(tool)) = (name(&from), name(&path)) {
                    found.push(State { source, tool, path });
                }
            }
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

/// 路径最后一段的名字。
fn name(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// 目录 `dir` 下一层的文件夹；读不了的没有。
fn folders(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

/// 卸掉来处是 `source` 的包（施工 X-44）：删掉每个账号下的 `state/scripts/<账号>/<来处>/`。本来就没有的不算错。
///
/// # Errors
///
/// 删不掉的那一个。
pub fn forget_states(data_root: &Path, source: &str) -> std::io::Result<()> {
    for account in folders(&state_root(data_root)) {
        match std::fs::remove_dir_all(account.join(source)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
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

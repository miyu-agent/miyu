//! 找斜杠命令模板（施工 X-14 上，`docs/blueprint/slash.md`「找、盖住、开关」）：两处找 `<名字>.md`：
//!
//! - 本机自己放的 `commands/`；
//! - 装了的软件包（清单读成了的）的 `commands/`，出厂的先于家目录、同一层照编号（同技能，`skills.rs`）。
//!
//! 同名的先找到的赢：自己放的盖住包里的，后来的跳过、记下被谁盖住。名字不合写法的、开头写错的、读不了的跳过，交回给调的
//! 一方记。子文件夹、点开头的、不是 `.md` 的不算。模板可以是链接（施工 X-37，`links.rs`）：照它指到的读。和核心内置
//! 的撞名由调的一方判。碰磁盘，调的一方放在阻塞线程里。

use std::path::{Path, PathBuf};

use miyu_policy::slash::{self, Template};

use crate::links::{self, Fence, Kind};
use crate::packages::Found;
use crate::root::DataRoot;
use crate::skills::Skipped;

/// 模板文件的扩展名。
const EXTENSION: &str = "md";

/// 找到的一个模板命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// 命令名：文件名去掉 `.md`。
    pub name: String,
    /// 读好的模板。
    pub template: Template,
    /// 模板文件。
    pub file: PathBuf,
    /// 哪个软件包带的；自己放的没有。
    pub package: Option<String>,
}

/// 找命令模板：装了的软件包 `packages` 里的，加数据根 `root` 里账号 `account` 自己放的。照名字排。
pub fn find(packages: &[Found], root: &DataRoot) -> (Vec<Command>, Vec<Skipped>) {
    let mut places: Vec<(PathBuf, Option<String>)> = vec![(root.path().join("commands"), None)];
    let mut installed: Vec<&Found> = packages.iter().filter(|found| found.read.is_ok()).collect();
    installed.sort_by(|a, b| (a.layer, &a.id).cmp(&(b.layer, &b.id)));
    places.extend(
        installed
            .into_iter()
            .map(|found| (found.files_dir().join("commands"), Some(found.id.clone()))),
    );
    let mut commands: Vec<Command> = Vec::new();
    let mut skipped = Vec::new();
    let fence = Fence::of(root);
    for (place, package) in places {
        let (files, unlisted) = files(&place, &fence);
        skipped.extend(unlisted);
        for (file, name) in files {
            match read(&file, &name) {
                Err(why) => skipped.push(Skipped::new(file, why)),
                Ok(template) => match commands.iter().find(|found| found.name == name) {
                    Some(winner) => skipped.push(Skipped {
                        by: Some(winner.file.clone()),
                        ..Skipped::new(
                            file,
                            format!(
                                "command {name:?} is already provided by {}",
                                winner.file.display()
                            ),
                        )
                    }),
                    None => commands.push(Command {
                        name,
                        template,
                        file,
                        package: package.clone(),
                    }),
                },
            }
        }
    }
    commands.sort_by(|a, b| a.name.cmp(&b.name));
    (commands, skipped)
}

/// 包文件夹 `dir` 带的命令名（装之前查冲突、给人看）：`commands/` 下读得成的，照名字排；读不成的另交回为什么。
pub fn carried(dir: &Path) -> (Vec<String>, Vec<Skipped>) {
    let mut names = Vec::new();
    let mut skipped = Vec::new();
    let (files, unlisted) = files(&dir.join("commands"), &Fence::anywhere());
    skipped.extend(unlisted);
    for (file, name) in files {
        match read(&file, &name) {
            Ok(_) => names.push(name),
            Err(why) => skipped.push(Skipped::new(file, why)),
        }
    }
    (names, skipped)
}

/// 读名字是 `name` 的模板文件 `file`：名字不合写法、读不了、开头写错的交回为什么。
fn read(file: &Path, name: &str) -> Result<Template, String> {
    if !slash::valid_name(name) {
        return Err(format!(
            "command name {name:?} must start with a lowercase letter and use only lowercase letters, digits and hyphens, at most 32"
        ));
    }
    let text = std::fs::read_to_string(file).map_err(|error| format!("not readable: {error}"))?;
    slash::read(&text).map_err(|problem| problem.to_string())
}

/// `place` 下面的 `.md` 文件和去掉扩展名的名字，照名字排；没有这个目录的是空的。点开头的、子文件夹不算；链接照 `fence`
/// 认（施工 X-37），断了的、指出去的另交回跳过。
fn files(place: &Path, fence: &Fence) -> (Vec<(PathBuf, String)>, Vec<Skipped>) {
    let (entries, skipped) = links::list(place, Kind::File, fence, |name| {
        Path::new(name).extension().and_then(|ext| ext.to_str()) == Some(EXTENSION)
    });
    let files = entries
        .into_iter()
        .filter_map(|entry| {
            let name = Path::new(&entry.name).file_stem()?.to_str()?.to_string();
            Some((entry.path, name))
        })
        .collect();
    (files, skipped)
}

#[cfg(test)]
mod tests;

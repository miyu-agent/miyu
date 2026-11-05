//! 软件包清单在哪、三层怎么认（施工 9-1 上，SD-1 改成三层，`docs/blueprint/packages.md`「在哪」「三层怎么认」）：一个文件夹
//! 就是一个包（施工 F-8 上，设计 `31-软件包.md` 第二节）：出厂的放资源目录的 `packages/<编号>/package.toml`，系统的包管理器另外
//! 装的放系统加装那几处（[`distro`]，设计 `60-系统源分包.md` 第三节），主人自己装的放数据根的 `opt/<编号>/package.toml`，包的
//! 文件都在这个文件夹里。一个包只认一份清单：同一个编号几层都有的，装上的盖过系统加装的、系统加装的盖过出厂的（系统加装
//! 几处之间先列的优先），被盖住的那几份记成 [`Issue::Covered`]，不算错、不加载。两个包要同一个子命令名、功能、前缀的，照盖的
//! 先后（装上的、系统加装的、出厂的，同一层照编号）先到先得，后到的那一份报 `command_taken` 这些。读法同配置文件：顺着链接
//! 读。文件怎么读成样子在 `miyu_config::package`。
//!
//! 包自己在这台机器上的状态放 `<数据根>/var/lib/<编号>/`，包自己建、自己用（[`Packages::state_dir`]）。
//!
//! 装、卸（施工 F-5 上）：只动家目录那一层（`install.rs`）。卸掉的出厂的、系统加装的包在 `etc/removed/` 记一笔 `<编号>`，文件不动，
//! 读的时候不算装了（[`Packages::read`]），另外照样读得出来（[`Packages::read_removed`]），好让头给人装回来。

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use miyu_config::package::{self, Code, Manifest, Problem};

use crate::resources::ResourceRoot;
use crate::root::DataRoot;

/// 正在跑的这一版 Miyu（施工 X-31）：读清单时照它比 `[package] miyu`（`miyu_config::package::read_for`）。
const RUNNING: &str = env!("CARGO_PKG_VERSION");

pub mod conflicts;
pub mod distro;
pub mod install;
mod launch;
pub mod leftovers;
pub mod local;
pub mod settings_left;

pub use launch::{Launch, first_line, launcher};
/// 编号是保留名字的那一条问题（表在 `miyu_config::package`，施工 FH-5下 挪过去）。
pub use miyu_config::package::reserved_id;

/// 清单在包文件夹里的名字（施工 F-8 上）。
pub const MANIFEST: &str = "package.toml";

/// 包带的程序放在包文件夹里的这个子目录（施工 F-8 上再补，设计 `31-软件包.md` 第二节）。
pub const BIN: &str = "bin";

/// 一层：清单从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// 随发行附带的，只读。
    Shipped,
    /// 系统的包管理器另外装的（施工 SD-1，[`distro`]）：只归系统包管理器管，我们不改它的文件。
    Distro,
    /// 主人装的：数据根的 `opt/`（协议上叫 `home`）。
    Home,
}

impl Layer {
    /// 协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Shipped => "shipped",
            Layer::Distro => "distro",
            Layer::Home => "home",
        }
    }
}

/// 读到的一份清单。
#[derive(Debug, Clone)]
pub struct Found {
    /// 编号：包文件夹的名字。
    pub id: String,
    /// 哪一层。
    pub layer: Layer,
    /// 清单在哪：包文件夹里的 `package.toml`。
    pub path: PathBuf,
    /// 读成的样子，或者问题。
    pub read: Result<Manifest, Issue>,
}

impl Found {
    /// 包自己的文件放在哪个目录（施工 R-5 三补，`packages.md`「在哪」）：包文件夹本身，清单就在里面，例如
    /// `packages/embed/package.toml` 的是 `packages/embed/`（施工 F-8 上）。两层都是这样。
    pub fn files_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf)
    }

    /// 怎么起这个包的程序 `program`：见 [`locate`]。
    pub fn program(&self, program: &str, main: &Path) -> Option<Launch> {
        locate(program, &self.files_dir(), main)
    }
}

/// 一份清单的问题。
#[derive(Debug)]
pub enum Issue {
    /// 写错了，或者和别的包撞了。
    Wrong(Problem),
    /// 读不了。
    Unreadable(io::Error),
    /// 同一个编号上面一层还有一份，这一份被它盖住（施工 SD-1）：不算错，不加载。`by` 是盖住它的那一层，`version` 是这一份
    /// 自己写的版本（读不成、没写的没有）。
    Covered {
        /// 盖住它的那一层。
        by: Layer,
        /// 这一份的版本。
        version: Option<String>,
    },
}

/// 拷一份：读不了的照种类和原话另造一个，别的照抄。
impl Clone for Issue {
    fn clone(&self) -> Issue {
        match self {
            Issue::Wrong(problem) => Issue::Wrong(problem.clone()),
            Issue::Unreadable(error) => {
                Issue::Unreadable(io::Error::new(error.kind(), error.to_string()))
            }
            Issue::Covered { by, version } => Issue::Covered {
                by: *by,
                version: version.clone(),
            },
        }
    }
}

/// 软件包清单的三层。
#[derive(Debug, Clone)]
pub struct Packages {
    dirs: Vec<(Layer, PathBuf)>,
    /// 放各包状态的目录（[`DataRoot::package_states`]）。
    states: PathBuf,
    /// 本地库（[`DataRoot::local_packages`]）。
    local: PathBuf,
    /// 卸掉的出厂包的记号（[`DataRoot::removed_markers`]）。
    removed: PathBuf,
}

impl Packages {
    /// 照资源目录 `resources`、数据根 `root` 定三层的位置：出厂的、系统加装的（`distro`，照先后，施工 SD-1）、装上的
    /// （`opt/`；协议上这一层照旧叫 `home`）。真跑的用 [`Packages::system`]，测试照要的写 `distro`。
    pub fn new(resources: &ResourceRoot, root: &DataRoot, distro: Vec<PathBuf>) -> Packages {
        let mut dirs = vec![(Layer::Shipped, resources.path().join("packages"))];
        dirs.extend(distro.into_iter().map(|dir| (Layer::Distro, dir)));
        dirs.push((Layer::Home, root.packages()));
        Packages {
            dirs,
            states: root.package_states(),
            local: root.local_packages(),
            removed: root.removed_markers(),
        }
    }

    /// 同 [`Packages::new`]，系统加装那几处照这个进程的环境算（[`distro::dirs`]）。
    pub fn system(resources: &ResourceRoot, root: &DataRoot) -> Packages {
        let distro = distro::dirs(
            &distro::Where::current(),
            &resources.path().join("packages"),
        );
        Packages::new(resources, root, distro)
    }

    /// 系统加装那几处，照先后。
    pub fn distro(&self) -> Vec<PathBuf> {
        self.dirs
            .iter()
            .filter(|(layer, _)| *layer == Layer::Distro)
            .map(|(_, dir)| dir.clone())
            .collect()
    }

    /// 只有出厂那一层（施工 9-1 下）：生成配置的样本照出厂带的包，和数据根无关。状态目录没有意义，是空的。
    pub fn shipped(resources: &ResourceRoot) -> Packages {
        Packages {
            dirs: vec![(Layer::Shipped, resources.path().join("packages"))],
            states: PathBuf::new(),
            local: PathBuf::new(),
            removed: PathBuf::new(),
        }
    }

    /// 三层里所有的清单，照编号排（同一个编号认的那一份在前，被盖住的接在后面）。目录读不了的一层当没有；不是文件夹的、编号
    /// 不合写法的、文件夹里没有 `package.toml` 的不算。
    /// 卸掉的出厂的、系统加装的包（施工 F-5 上、SD-1，数据根里记了一笔的）不在里面。
    pub fn read(&self) -> Vec<Found> {
        let removed = self.removed();
        let found: Vec<Found> = self
            .dirs
            .iter()
            .flat_map(|(layer, dir)| {
                files(dir)
                    .into_iter()
                    .map(move |(id, path)| (*layer, id, path))
            })
            .filter(|(layer, id, _)| !(*layer != Layer::Home && removed.contains(id)))
            .map(|(layer, id, path)| read_one(layer, id, path))
            .collect();
        self.checked(found)
    }

    /// 照编号排（同一个编号盖得住的在前：装上的、系统加装的、出厂的，系统加装几处照先后），认盖、撞（[`Packages::read`]）。
    /// 排序是稳定的：系统加装几处之间照读进来的先后（也就是列的先后）。
    fn checked(&self, mut found: Vec<Found>) -> Vec<Found> {
        found.sort_by(|one, other| {
            (&one.id, std::cmp::Reverse(one.layer))
                .cmp(&(&other.id, std::cmp::Reverse(other.layer)))
        });
        covered(&mut found);
        reserved(&mut found);
        taken(&mut found);
        features_taken(&mut found);
        prefixes_taken(&mut found);
        found
    }

    /// 卸掉的出厂的、系统加装的包（施工 F-5 上、SD-1）：数据根里记了一笔、这两层有清单的，照编号排，一个编号一份（照盖的先后
    /// 取盖得住的那一份），各自读成样子（不和别的包比撞没撞）。
    pub fn read_removed(&self) -> Vec<Found> {
        let removed = self.removed();
        let mut picked: BTreeMap<String, (Layer, PathBuf)> = BTreeMap::new();
        // 照盖的先后看：系统加装的（照列的先后）先于出厂的，先看到的留下。
        let order = self
            .dirs
            .iter()
            .filter(|(layer, _)| *layer == Layer::Distro)
            .chain(
                self.dirs
                    .iter()
                    .filter(|(layer, _)| *layer == Layer::Shipped),
            );
        for (layer, dir) in order {
            for (id, path) in files(dir) {
                if removed.contains(&id) && !picked.contains_key(&id) {
                    picked.insert(id, (*layer, path));
                }
            }
        }
        picked
            .into_iter()
            .map(|(id, (layer, path))| Found {
                read: std::fs::read_to_string(&path)
                    .map_err(Issue::Unreadable)
                    .and_then(|text| package::read_for(&text, RUNNING).map_err(Issue::Wrong)),
                id,
                layer,
                path,
            })
            .collect()
    }

    /// 本地库在哪（施工 F-8 中上，[`local`]；位置照 [`DataRoot::local_packages`]）。
    pub fn local_root(&self) -> PathBuf {
        self.local.clone()
    }

    /// 家目录那一层的目录（施工 F-5 上：装、卸只动它）；只有出厂那一层的没有。
    pub fn home_dir(&self) -> Option<&Path> {
        self.dirs
            .iter()
            .find(|(layer, _)| *layer == Layer::Home)
            .map(|(_, dir)| dir.as_path())
    }

    /// 记着卸掉的出厂的、系统加装的包的编号（施工 F-5 上、SD-1）。只有出厂那一层的没有。
    pub fn removed(&self) -> Vec<String> {
        if self.removed.as_os_str().is_empty() {
            return Vec::new();
        }
        install::removed(&self.removed)
    }

    /// 卸掉的出厂包的记号放在哪（[`DataRoot::removed_markers`]）：卸、装回出厂的包写它。
    pub fn removed_markers(&self) -> PathBuf {
        self.removed.clone()
    }

    /// 三层的目录：`miyu check` 认写了的文件是不是一份清单（两边换成真的路径比，`miyu-endpoint` 的 `check.rs`）。
    pub fn dirs(&self) -> impl Iterator<Item = (Layer, &Path)> {
        self.dirs.iter().map(|(layer, dir)| (*layer, dir.as_path()))
    }

    /// 包 `id` 在这台机器上放状态的目录（位置照 [`DataRoot::package_state`]）。不建：包自己建。
    pub fn state_dir(&self, id: &str) -> PathBuf {
        self.states.join(id)
    }
}

/// 怎么起包里的程序 `program`（施工 9-2，`packages.md`「转交」）：先找包文件夹 `dir` 里的 `bin/`（施工 F-8 上再补：别人做的包
/// 自己带程序，装的时候随包拷进来），再找主程序 `main` 真实位置旁边的（随 Miyu 装的那几个；和 `miyu web` 找 `miyu-web` 一个
/// 办法）；Windows 加 `.exe`。Windows 上这两处都没有的，再找 `bin/<program>`（没有后缀），照它第一行的 `#!` 起（施工 X-34，
/// [`launcher`]），没有 `#!` 的不算；Linux、macOS 上 `bin/<program>` 本来就是第一处，系统认 `#!`。都没有的是没有。程序本身不找
/// `PATH`：别的程序冒充不了（2026-10-01 项目主人定，第二节「拆 M9 时另带四样」第 4 条）；照 `#!` 找的解释器照名字找。
pub fn locate(program: &str, dir: &Path, main: &Path) -> Option<Launch> {
    locate_on(program, dir, main, cfg!(windows))
}

/// 同 [`locate`]，`windows` 是 Windows 上的找法（测试在别的平台上也走一遍）。
fn locate_on(program: &str, dir: &Path, main: &Path, windows: bool) -> Option<Launch> {
    let suffix = if windows { ".exe" } else { "" };
    let file = format!("{program}{suffix}");
    // 相对的不认：那是照当前目录找，谁在当前目录放一个 `bin/` 都能冒充。
    let own = dir.join(BIN).join(&file);
    if dir.is_absolute() && own.is_file() {
        return Some(Launch::direct(own));
    }
    let real = std::fs::canonicalize(main).unwrap_or_else(|_| main.to_path_buf());
    let beside = real.parent().map(|dir| dir.join(&file));
    if let Some(beside) = beside.filter(|path| path.is_file()) {
        return Some(Launch::direct(beside));
    }
    let script = dir.join(BIN).join(program);
    if !windows || !dir.is_absolute() || !script.is_file() {
        return None;
    }
    // `#!` 后面什么都没写的，照它起的还是文件本身，Windows 上起不了：当没有。
    let first = first_line(&script).ok()?;
    first
        .starts_with("#!")
        .then(|| launcher(&script, &first, true))
        .filter(|launch| launch.program != script)
}

/// 一层目录里的清单：编号（包文件夹的名字）和清单的路径。读不了的目录当没有；点开头的（装到一半的暂存、备份）编号不合
/// 写法，不算。
fn files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let folder = entry.path();
            let id = folder.file_name()?.to_str()?.to_string();
            let manifest = folder.join(MANIFEST);
            (crate::personas::valid(&id) && manifest.is_file()).then_some((id, manifest))
        })
        .collect()
}

/// 同一个编号：排在前面的那一份认（[`Packages::checked`] 照盖的先后排好了），后面的几份记成被它盖住（施工 SD-1，原来的
/// `duplicate` 去掉）。
fn covered(found: &mut [Found]) {
    for index in 1..found.len() {
        let winner = (0..index)
            .rev()
            .take_while(|&before| found[before].id == found[index].id)
            .last();
        if let Some(winner) = winner {
            let version = match &found[index].read {
                Ok(manifest) => manifest.version.clone(),
                Err(_) => None,
            };
            found[index].read = Err(Issue::Covered {
                by: found[winner].layer,
                version,
            });
        }
    }
}

/// 编号是数据根里保留的名字的（[`crate::root::RESERVED`]）整份不收，报 `reserved_id`：它的东西会和数据根自己的目录撞上。
fn reserved(found: &mut [Found]) {
    for one in found.iter_mut() {
        if let Some(problem) = reserved_id(&one.id) {
            one.read = Err(Issue::Wrong(problem));
        }
    }
}

/// 读的先后，照盖的先后（施工 SD-1）：装上的、系统加装的、出厂的，同一层照编号（系统加装几处之间照列的先后，排序是稳定的）。
/// 先到先得的几样（子命令名、功能的编号、前缀）照它。
fn reading_order(found: &[Found]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..found.len()).collect();
    order.sort_by(|&one, &other| {
        (std::cmp::Reverse(found[one].layer), &found[one].id)
            .cmp(&(std::cmp::Reverse(found[other].layer), &found[other].id))
    });
    order
}

/// 子命令名：照读的先后先到先得，后到的那一份报 `command_taken`，报在被占的那个子命令名那一行；一个包加几个的（施工 F-13）
/// 有一个被占就整份不收，它的几个名字都不算占了。
fn taken(found: &mut [Found]) {
    let order = reading_order(found);
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for index in order {
        let Ok(manifest) = &found[index].read else {
            continue;
        };
        let clash = manifest.commands.iter().find_map(|command| {
            owners
                .get(&command.name)
                .map(|owner| (command.name.clone(), command.line, owner.clone()))
        });
        match clash {
            None => {
                let id = found[index].id.clone();
                for command in &manifest.commands {
                    owners.insert(command.name.clone(), id.clone());
                }
            }
            Some((name, line, owner)) => {
                let message = format!("command {name} is already taken by package {owner}");
                found[index].read = Err(Issue::Wrong(Problem {
                    line,
                    code: Code::CommandTaken,
                    detail: name,
                    message,
                }));
            }
        }
    }
}

/// 功能的编号（施工 F-1，设计 30 第三节）：照读的先后先到先得，后到的那一份报 `feature_taken`、整份不收，报在那个功能那一行；
/// 没写功能的包照包的编号算（[`miyu_config::package::Manifest::features_of`]）。
fn features_taken(found: &mut [Found]) {
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for index in reading_order(found) {
        let Ok(manifest) = &found[index].read else {
            continue;
        };
        let features = manifest.features_of(&found[index].id);
        let clash = features.iter().find_map(|feature| {
            owners
                .get(&feature.id)
                .map(|owner| (feature, owner.clone()))
        });
        match clash {
            Some((feature, owner)) => {
                let message = format!("feature {} is already taken by package {owner}", feature.id);
                found[index].read = Err(Issue::Wrong(Problem {
                    line: feature.line,
                    code: Code::FeatureTaken,
                    detail: feature.id.clone(),
                    message,
                }));
            }
            None => {
                for feature in features {
                    owners.insert(feature.id, found[index].id.clone());
                }
            }
        }
    }
}

/// 前缀（施工 PN-1，设计 48 第二节第 3 条）：照读的先后先到先得，后到的那一份报 `prefix_taken`、整份不收；没写的照包编号算。
/// 装的时候先查过（`conflicts`），读的时候撞得上的只剩核心升级后出厂的新带了同一个前缀的：出厂的先读到。
fn prefixes_taken(found: &mut [Found]) {
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for index in reading_order(found) {
        let Ok(manifest) = &found[index].read else {
            continue;
        };
        let prefix = manifest.prefix_of(&found[index].id).to_string();
        match owners.get(&prefix) {
            Some(owner) => {
                let message = format!("prefix {prefix} is already taken by package {owner}");
                found[index].read = Err(Issue::Wrong(Problem {
                    line: None,
                    code: Code::PrefixTaken,
                    detail: prefix,
                    message,
                }));
            }
            None => {
                owners.insert(prefix, found[index].id.clone());
            }
        }
    }
}

/// 读一份：层 `layer`、编号 `id`、清单在 `path`。
fn read_one(layer: Layer, id: String, path: PathBuf) -> Found {
    Found {
        read: std::fs::read_to_string(&path)
            .map_err(Issue::Unreadable)
            .and_then(|text| package::read_for(&text, RUNNING).map_err(Issue::Wrong)),
        id,
        layer,
        path,
    }
}

#[cfg(test)]
mod tests;

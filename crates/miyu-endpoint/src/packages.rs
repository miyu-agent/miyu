//! 软件包清单（施工 9-1 上，`docs/blueprint/packages.md`）：核心起来时读一次（[`load`]），`package.list` 照它答；装、卸、
//! 改了清单要重启核心才认。`miyu check` 照磁盘上现在的查（`check.rs`）。名字、说明照连接的语言挑，挑法同 `persona.list`；
//! 写错的、撞了的、协议版本对不上的带代码和给人看的一句。

use serde_json::{Value, json};

use std::collections::BTreeSet;

use miyu_config::package::{Code, Manifest, PackageKind, Problem, settings};
use miyu_config::{Item, Words};
use miyu_policy::features::{Feature, Features};
use miyu_store::human::Human;
use miyu_store::packages::{Found, Issue, Packages};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::Core;

mod archive;
pub(crate) mod asked;
pub(crate) mod changes;
pub(crate) mod depends;
pub(crate) mod fill;
mod folder;
pub(crate) mod group;
mod info;
mod item;
pub(crate) mod left;
pub(crate) mod local;
pub(crate) mod logo;
pub(crate) mod manage;
mod may_manage;
pub(crate) mod methods;
pub(crate) mod off;
pub(crate) mod own;
pub(crate) mod port;
pub(crate) mod preview;
pub(crate) mod progress;
mod services;
pub(crate) mod source;
pub(crate) mod status;
pub(crate) mod switch;
pub(crate) mod verify;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;
use item::listed;
pub use status::absent;

/// 编号 `id` 的包文件夹：核心手里这时的清单里的（`Found::files_dir`）。没有的交回空路径，照它找不到包自己带的程序（施工
/// F-8 上再补：程序先找包文件夹的 `bin/`）。
pub(crate) fn dir_of(core: &Core, id: &str) -> std::path::PathBuf {
    core.system_packages()
        .iter()
        .find(|one| one.id == id)
        .map_or_else(std::path::PathBuf::new, Found::files_dir)
}

/// 核心手里的包 `id` 要程序、程序不在（[`status::program_missing`]）。
pub(crate) fn program_missing(core: &Core, id: &str, manifest: &Manifest) -> bool {
    status::program_missing(&dir_of(core, id), manifest)
}

/// 核心说的协议主版本。
const PROTOCOL: u32 = 1;

/// 运行日志的目标。
const TARGET: &str = "miyu::packages";

/// 这个核心的两层清单。
pub(crate) fn packages(core: &Core) -> Packages {
    Packages::new(&core.resources, &core.root)
}

/// 核心起来时读一次：写错的、撞了的各记一行 `WARN package invalid`，读不了的 `WARN package unreadable`。装包的目录没有的先建
/// 好（施工 U23 下上）。
pub fn load(resources: &ResourceRoot, root: &DataRoot) -> Vec<Found> {
    if let Err(error) = std::fs::create_dir_all(root.packages()) {
        tracing::warn!(target: TARGET, error = %error, "packages not created");
    }
    let packages = Packages::new(resources, root);
    let found = packages.read();
    // 本地库（施工 F-8 中上）：以前装的、还没记的补一份。
    local::backfill(&packages, &found);
    for one in &found {
        match &one.read {
            Ok(_) => {}
            Err(Issue::Wrong(problem)) => {
                tracing::warn!(target: TARGET, package = one.id.as_str(), file = %one.path.display(), error = %problem, "package invalid");
            }
            Err(Issue::Unreadable(error)) => {
                tracing::warn!(target: TARGET, package = one.id.as_str(), file = %one.path.display(), error = %error, "package unreadable");
            }
        }
    }
    found
}

/// `package.list`：核心这时认的（起来时读的，装卸以后当场换，施工 F-5 上），照编号排；卸掉的出厂的接在后面，带
/// `removed: true`（施工 F-5 上），好让头给人装回来。
pub(crate) fn list(core: &Core, peer: Peer) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let places = packages(core);
    let seen = core.system_packages();
    let mut items: Vec<Value> = seen
        .iter()
        .map(|found| listed(core, found, (&places, &seen), &words, peer.language, false))
        .collect();
    items.extend(
        places
            .read_removed()
            .iter()
            .filter(|found| !seen.iter().any(|one| one.id == found.id))
            .map(|found| listed(core, found, (&places, &seen), &words, peer.language, true)),
    );
    if !peer.for_owner {
        for item in &mut items {
            for_others(item);
        }
    }
    Ok(json!({ "packages": items, "groups": group::listed(core, peer.language) }))
}

/// 列表里包 `id` 那一项，照 `peer` 的语言（施工 F-8 三补：推送照它算）；
/// 装着的、卸掉了的出厂的都有，都不是的没有。
pub(crate) fn entry(core: &Core, id: &str, peer: Peer) -> Option<Value> {
    let words = words(core, peer.language).ok()?;
    let places = packages(core);
    let seen = core.system_packages();
    let mut item = match seen.iter().find(|found| found.id == id) {
        Some(found) => listed(core, found, (&places, &seen), &words, peer.language, false),
        None => {
            let removed = places.read_removed();
            let found = removed.iter().find(|found| found.id == id)?;
            listed(core, found, (&places, &seen), &words, peer.language, true)
        }
    };
    // 同 [`list`]：不替主人干活的裁掉只给主人的那几格（施工 U23-R修二）。
    if !peer.for_owner {
        for_others(&mut item);
    }
    Some(item)
}

/// 只给主人的那几格（施工 U23-R）：这台机器上的路径（`state`、`pages_dir`）、程序和参数（`command`、`commands`、`process`、
/// `worker`、`check`）、MCP 服务的写法（可能带环境变量、请求头）、读坏了的原话（`problem`、`line`）。不替主人干活的连接（场所里
/// 替外人的平台桥）只拿到别的：它照 `export`、`connection` 这些找挂在自己身上的包。
const OWNER_ONLY: &[&str] = &[
    "state",
    "pages_dir",
    "command",
    "commands",
    "process",
    "worker",
    "check",
    "problem",
    "line",
];

/// 把一项里 [`OWNER_ONLY`] 那几格去掉。带的 MCP 服务只留名字、状态、停下的原因（施工 U23-R修三），怎么起（命令、网址、环境变量、
/// 请求头、标准错误）只给主人。
fn for_others(item: &mut Value) {
    if let Some(fields) = item.as_object_mut() {
        fields.retain(|key, _| !OWNER_ONLY.contains(&key.as_str()));
        if let Some(Value::Array(servers)) = fields.get_mut("mcp") {
            for server in servers.iter_mut() {
                if let Some(server) = server.as_object_mut() {
                    server.retain(|key, _| matches!(key.as_str(), "name" | "state" | "reason"));
                }
            }
        }
    }
}

/// 说的协议版本不包含核心的：交回给人看的那个范围，例如 `2–3`；包含的、不说协议的（吉祥物包）没有。
pub(crate) fn mismatch(manifest: &Manifest) -> Option<String> {
    let [low, high] = manifest.protocol?;
    if (low..=high).contains(&PROTOCOL) {
        return None;
    }
    Some(if low == high {
        low.to_string()
    } else {
        format!("{low}–{high}")
    })
}

/// 包的配置项（施工 9-1 下，`packages.md`「配置项」）：读成了的清单的 `[settings]` 拼成配置项，键是 `<前缀>.<名字>`（施工
/// PN-1：清单 `prefix`，没写的照包编号）。前缀是核心自己的某个模块（`core_items` 里键的第一段，「软件包」那一页的不算）的，
/// 那一份改报 `settings_taken`、一项都不收。
/// 核心起来时调一次：拼出来的项一直用到退出（`miyu_config::package::settings::items`）。
pub fn settle(found: &mut [Found], core_items: &[Item]) -> Vec<Item> {
    settle_with(found, core_items, &|one| !absent(one))
}

/// 同 [`settle`]，程序在不在照 `present` 认：生成配置的样本、测出厂的清单时照发行包的样子算，程序都在 `miyu` 旁边（施工
/// F-6 上：不照跑测试的这台机器）。
pub fn settle_with(
    found: &mut [Found],
    core_items: &[Item],
    present: &dyn Fn(&Found) -> bool,
) -> Vec<Item> {
    let modules: BTreeSet<&str> = core_items
        .iter()
        // 「软件包」那一页的是包的：`miyu check` 照起来以后的清单认，里面已经有包的项。
        .filter(|item| item.ui.page != settings::PAGE)
        .filter_map(|item| item.key.split('.').next())
        .collect();
    let mut items = Vec::new();
    for one in found.iter_mut() {
        let Ok(manifest) = &one.read else {
            continue;
        };
        // 每个人能把它对自己整个关掉（施工 U23 末上）：「软件包」页这个包那一组底下一个开关。
        items.push(miyu_session::packages_off::item_for(&one.id));
        if manifest.settings.is_empty() {
            continue;
        }
        let prefix = manifest.prefix_of(&one.id).to_string();
        if modules.contains(prefix.as_str()) {
            one.read = Err(Issue::Wrong(Problem {
                line: None,
                code: Code::SettingsTaken,
                message: format!(
                    "the prefix {prefix} of package {} is a core module and cannot declare settings",
                    one.id
                ),
                detail: prefix,
            }));
            continue;
        }
        // 程序不在的扩展、小程序当没装，配置项不进（施工 F-6 上，`package-pages.md`「程序不在就当没装」）。
        if !present(one) {
            continue;
        }
        // 一律在「软件包」那一页、这个包那一组（施工 F-6 上：「接入」页去掉了）。
        items.extend(settings::items(&one.id, &prefix, &manifest.settings));
    }
    items
}

/// 清单是内置包、`built_in` 里没有的（施工 F-2，设计 30 第二节第 3 条）：这一份核心没编进它的代码，照读坏了的清单报
/// `not_built_in`、记一行 `WARN package invalid`。核心起来时读完清单调一次（`miyu-core`）。
pub fn compiled(found: &mut [Found], built_in: &[&str]) {
    for one in found.iter_mut() {
        let lacking = one.read.as_ref().is_ok_and(|manifest| {
            manifest.kind == PackageKind::Builtin && !built_in.contains(&one.id.as_str())
        });
        if !lacking {
            continue;
        }
        let problem = Problem {
            line: None,
            code: Code::NotBuiltIn,
            detail: one.id.clone(),
            message: format!(
                "this core does not have the code of built-in package {}",
                one.id
            ),
        };
        tracing::warn!(target: TARGET, package = one.id.as_str(), file = %one.path.display(), error = %problem, "package invalid");
        one.read = Err(Issue::Wrong(problem));
    }
}

impl Core {
    /// 照核心起来时核对过的清单 `packages` 换上（[`Core::with_packages`]）：人格记忆装没装（施工 F-2）、知识库带的包（施工
    /// K-8，只换不清：没收的包不是卸了）。
    pub(crate) fn adopt_packages(&self, packages: &[Found]) {
        self.memory
            .set_installed(is_installed(packages, crate::memory::PACKAGE));
        self.knowledge
            .use_packages(crate::knowledge::carried(packages));
    }
}

/// 包在哪几个组里（施工 BS-1 上中，设计 44 第三节第 2 条）：读成了的清单里写了 `[package] groups` 的，包的编号到组名。找预设时
/// 照它把 `[software]` 里写的组名分给组里的包（`miyu_store::presets::Presets::with_groups`）。
pub fn groups(found: &[Found]) -> std::collections::BTreeMap<String, Vec<String>> {
    found
        .iter()
        .filter_map(|one| {
            let manifest = one.read.as_ref().ok()?;
            (!manifest.groups.is_empty()).then(|| (one.id.clone(), manifest.groups.clone()))
        })
        .collect()
}

/// 功能在哪个包里（施工 PN-2）：读成了的清单里每个功能（[`miyu_config::package::Manifest::features_of`]）的编号到包的编号。
pub fn homes(found: &[Found]) -> std::collections::BTreeMap<String, String> {
    found
        .iter()
        .filter_map(|one| Some((one.id.as_str(), one.read.as_ref().ok()?)))
        .flat_map(|(id, manifest)| {
            manifest
                .features_of(id)
                .into_iter()
                .map(move |feature| (feature.id, id.to_string()))
        })
        .collect()
}

/// 内置包 `id` 装了（施工 F-2）：有它读成了的清单，种类是内置。编进来的代码照它起不起。
pub fn is_installed(found: &[Found], id: &str) -> bool {
    found.iter().any(|one| {
        one.id == id
            && one
                .read
                .as_ref()
                .is_ok_and(|manifest| manifest.kind == PackageKind::Builtin)
    })
}

/// 带人格、预设的那一层的几个包文件夹（施工 P-11 上，设计 31 第十节）：看得到的（施工 U23 上下，
/// [`Core::packages_of`]）读成了的装着的包，出厂的在前、同一层照编号。
pub(crate) fn folders(core: &Core) -> Vec<std::path::PathBuf> {
    let found = core.packages_of();
    let mut read: Vec<&Found> = found.iter().filter(|one| one.read.is_ok()).collect();
    read.sort_by(|a, b| (a.layer, &a.id).cmp(&(b.layer, &b.id)));
    read.iter().map(|one| one.files_dir()).collect()
}

/// 交给会话的「这一刻装了的软件包」（施工 X-11 下，`miyu_session::Installed`）：看得到的（施工 U23 上下，
/// [`Core::packages_of`]）。拿着核心的弱引用，会话活在核心里面，拿强的会绕成环；核心没了的交空的。
pub(crate) fn installed(core: &std::sync::Arc<Core>) -> miyu_session::Installed {
    let core = std::sync::Arc::downgrade(core);
    std::sync::Arc::new(move || {
        core.upgrade().map_or_else(
            || std::sync::Arc::new(Vec::new()),
            |core| core.packages_of(),
        )
    })
}

/// 装了的功能（施工 F-3 上，设计 30 第三节）：读成了的内置包、扩展包带的，照清单读的先后（包照编号，包里照写的先后）；
/// 没写功能的包整个算一个（[`Manifest::features_of`]）。界面、小程序不带。预设照它开关、工具照它归。清单写了
/// `needs = ["images"]` 的包另记一份（施工 X-25），接平台的包和它接的平台另记一份（施工 IM-4）。
pub fn features(found: &[Found]) -> Features {
    let connections = found
        .iter()
        .filter_map(|one| {
            let connection = one.read.as_ref().ok()?.connection.as_ref()?;
            Some((
                one.id.clone(),
                connection.platforms.iter().cloned().collect(),
            ))
        })
        .collect();
    let images = found
        .iter()
        .filter_map(|one| one.read.as_ref().ok().map(|manifest| (one, manifest)))
        .filter(|(_, manifest)| manifest.needs.contains(&miyu_config::package::Need::Images))
        .map(|(one, _)| one.id.clone());
    let images: Vec<String> = images.collect();
    Features::new(
        found
            .iter()
            .filter_map(|one| one.read.as_ref().ok().map(|manifest| (one, manifest)))
            // 程序不在的扩展当没装，它的功能不算（施工 F-6 上）。
            .filter(|(one, _)| !absent(one))
            .flat_map(|(one, manifest)| {
                manifest
                    .features_of(&one.id)
                    .into_iter()
                    .map(|feature| Feature {
                        id: feature.id,
                        package: one.id.clone(),
                        tools: feature.tools,
                        service: feature.service,
                        script: None,
                    })
            })
            .collect(),
    )
    .needing_images(images)
    .connecting(connections)
}

/// 读成了的清单：编号和样子（给人看的字照它并进包的配置项的名字）。
pub(crate) fn manifests(found: &[Found]) -> impl Iterator<Item = (&str, &Manifest)> {
    found.iter().filter_map(|one| match &one.read {
        Ok(manifest) => Some((one.id.as_str(), manifest)),
        Err(_) => None,
    })
}

/// 给人看的那一句：`package-problems/<code>`，`detail` 换进去。
pub(crate) fn sentence(words: &Human, code: &str, detail: &str) -> Option<String> {
    Words::sentence(
        words,
        &format!("package-problems/{code}"),
        &[("detail", detail)],
    )
}

#[cfg(test)]
mod tests;

//! 装、卸之前看一眼（施工 F-8 下补，设计 `31-软件包.md` 第一节、定了的 B）：`package.install`、`package.remove` 带
//! `preview: true` 的，照真做之前查的那几样查一遍（写错的、撞了的、必需的照样拒），交回要做什么，什么都不动。命令行照它印一份、
//! 问「继续？[Y/n]」（`cli/pkg.md`）。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::packages::Layer;
use miyu_store::packages::local;

use super::folder::source;
use super::manage::{clear_of_conflicts, removable, restored_clear_of_conflicts, setting_keys};
use super::packages;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 从包文件夹 `path` 装：编号、版本、换下哪一份（同编号的）、带了什么、要什么能力、几个文件多大。从哪来是 `file`（施工 PK-3 上：
/// 本机文件夹、包文件都是，从软件源缓存里拿 `.miyu` 手装的也是：核心看不出它原来是哪一条），能力没有 `held`（来路算变了，前面
/// 批过的不接着用），另有「注意」。
pub(crate) async fn installing(
    core: &Arc<Core>,
    peer: Peer,
    path: &Path,
) -> Result<Value, Refusal> {
    let (mut answer, _) = looked(core, peer, path).await?;
    answer.insert("source".into(), json!("file"));
    let id = answer["package"].as_str().unwrap_or_default().to_string();
    let item = super::notice::Item {
        id: &id,
        source: "file",
    };
    if let Some(said) = super::notice::notice(core, peer.language, &[item], true) {
        answer.insert("notice".into(), said);
    }
    Ok(Value::Object(answer))
}

/// 同 [`installing`]，另交回读成了的清单：`package.info` 带 `path` 的照它补名字、说明、依赖（施工 IN-4，`file_info.rs`）。
pub(super) async fn looked(
    core: &Arc<Core>,
    peer: Peer,
    path: &Path,
) -> Result<(Map<String, Value>, Manifest), Refusal> {
    let (folder, id, manifest) = source(core, peer, path)?;
    clear_of_conflicts((core, peer), &id, manifest.clone(), folder.clone()).await?;
    let (files, content) =
        tokio::task::spawn_blocking(move || (local::scan(&folder), Content::of(&folder)))
            .await
            .map_err(|_| Refusal::INTERNAL)?;
    let files = files.map_err(|_| Refusal::PATH_UNREADABLE)?;
    let mut answer = carries(core, peer, &id, &manifest, &content);
    let now = core.system_packages();
    if let Some(old) = now.iter().find(|one| one.id == id) {
        let version = old.read.as_ref().ok().and_then(|old| old.version.clone());
        answer.insert("replaces".into(), json!({ "version": version }));
    }
    answer.insert("files".into(), json!(files.len()));
    answer.insert("size".into(), json!(local::size(&files)));
    Ok((answer, manifest))
}

/// 把卸掉的出厂的、系统加装的包装回来：编号、版本、带了什么，`restores: true`。没卸过的 `unknown_package`。看的是装回来的那一份
/// （两层都有的取系统加装的，施工 SD-1补）。
pub(crate) async fn restoring(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    if !packages(core).removed().iter().any(|removed| removed == id) {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    restored_clear_of_conflicts((core, peer), id).await?;
    let removed = packages(core).read_removed();
    let one = removed
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let manifest = one.read.as_ref().map_err(|_| Refusal::UNKNOWN_PACKAGE)?;
    let mut answer = carries(core, peer, id, manifest, &Content::of(&one.files_dir()));
    answer.insert("restores".into(), json!(true));
    // 从哪来（施工 PK-3 上）：出厂的 `shipped`，系统加装的 `distro`。
    answer.insert("source".into(), json!(one.layer.as_str()));
    Ok(Value::Object(answer))
}

/// 打开装着的包之前看一眼（施工 F-10 下下，她的 `package` 问人的卡片）：带了什么、要什么能力，同装之前那一份。没有开关的、
/// 程序不在的照 `package.enable` 拒。
pub(crate) fn enabling(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let (manifest, dir) = super::switch::switchable(core, id)?;
    Ok(Value::Object(carries(
        core,
        peer,
        id,
        &manifest,
        &Content::of(&dir),
    )))
}

/// 装着的包带了什么、要什么能力（施工 IN-2，`package.info` 装着的那一份）：同装之前看一眼的「包含」那几格（`program`、`command`、
/// `page`、`skills`……）和 `capabilities`。碰磁盘：照包目录 `dir` 找技能、脚本。
pub(crate) fn provided(
    core: &Core,
    peer: Peer,
    (id, manifest): (&str, &Manifest),
    dir: &Path,
) -> (Value, Option<Value>) {
    let mut carried = carries(core, peer, id, manifest, &Content::of(dir));
    let capabilities = carried.remove("capabilities");
    for key in ["package", "version", "skipped"] {
        carried.remove(key);
    }
    (Value::Object(carried), capabilities)
}

/// 卸：编号、哪一层、版本；家目录里的几个文件多大；一并删掉的配置项（`purge` 的才有，配置里写了的键）、状态目录在哪多大（施工 PK-4，
/// 没有的不写）；配置目录在哪、配置里它有几项设置、留不留（施工 PK-5、PK-5补：不 `purge` 的留，[`config_entry`]）。
pub(crate) async fn removing(core: &Arc<Core>, id: &str, purge: bool) -> Result<Value, Refusal> {
    let found = core.system_packages();
    let one = removable(&found, id)?;
    let mut answer = Map::new();
    answer.insert("package".into(), json!(id));
    answer.insert("layer".into(), json!(one.layer.as_str()));
    let version = one.read.as_ref().ok().and_then(|it| it.version.clone());
    if let Some(version) = version {
        answer.insert("version".into(), json!(version));
    }
    if one.layer == Layer::Home {
        let looked = super::local::entry(core, id).await?;
        answer.insert("files".into(), json!(looked.files.len()));
        answer.insert("size".into(), json!(local::size(&looked.files)));
    }
    let settings = crate::config::forget::present(core, &setting_keys(one));
    // 顶上的 `settings` 照旧是「一并删掉的配置项」（施工 PK-5补）：不 `purge` 的留着、是空的，没跟着改的头不会说错成删掉。
    let deleted: &[String] = if purge { &settings } else { &[] };
    answer.insert("settings".into(), json!(deleted));
    // 状态目录（施工 PK-4）：在哪、多大，好让人知道卸掉会丢什么；没有的不写。量大小要走一遍目录，放在阻塞线程里。配置目录
    // （施工 PK-5）：在哪、照 `purge` 留下还是删掉；没东西可留的不写，同一个阻塞线程里看。
    let (state, config) = (core.root.package_state(id), core.root.package_config(id));
    let (measured, config) = tokio::task::spawn_blocking(move || {
        let measured = measured(&state).map(|size| (state, size));
        (measured, holds_something(&config).then_some(config))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    if let Some((path, size)) = measured {
        answer.insert(
            "state".into(),
            json!({"path": path.to_string_lossy(), "size": size}),
        );
    }
    if let Some(entry) = config_entry(config.as_deref(), settings.len(), purge) {
        answer.insert("config".into(), entry);
    }
    Ok(Value::Object(answer))
}

/// 看一眼的 `config` 那一格（施工 PK-5、PK-5补）：`{path?, kept, settings?}`。`path` 是有东西可留、可删的配置目录，`settings` 是
/// 配置里它写了几项设置（0 的不写），`kept` 是 `!purge`：两样一起留下或者一起删掉。两样都没有的整格不写。
pub(super) fn config_entry(path: Option<&Path>, settings: usize, purge: bool) -> Option<Value> {
    if path.is_none() && settings == 0 {
        return None;
    }
    let mut entry = Map::new();
    if let Some(path) = path {
        entry.insert("path".into(), json!(path.to_string_lossy()));
    }
    entry.insert("kept".into(), json!(!purge));
    if settings > 0 {
        entry.insert("settings".into(), json!(settings));
    }
    Some(Value::Object(entry))
}

/// 配置目录 `dir` 里有没有东西可留、可删（施工 PK-5）：是目录、底下至少一项；本身是链接的也算（留着的是那个链接，`purge`
/// 删的也只是链接，同 [`measured`] 不跟链接）。不在的、空的、是普通文件的不算：空的卸的时候删掉（`manage.rs` 的 `drop_if_empty`），
/// 普通文件不是包的配置目录。
pub(super) fn holds_something(dir: &Path) -> bool {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_symlink() => true,
        Ok(meta) if meta.is_dir() => {
            std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
        }
        _ => false,
    }
}

/// 卸包会删掉的状态目录 `dir` 有多大（施工 PK-4）：底下普通文件的字节数之和。不跟符号链接（卸的时候只删链接本身，指到的
/// 东西不动）：`dir` 自己是链接的算 0，底下的链接不算、不进去；读不了的子目录、文件跳过。`dir` 不在、是普通文件的是 `None`。
fn measured(dir: &Path) -> Option<u64> {
    let kind = std::fs::symlink_metadata(dir).ok()?.file_type();
    if kind.is_symlink() {
        return Some(0);
    }
    if !kind.is_dir() {
        return None;
    }
    let mut total = 0;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(at) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            // `DirEntry::file_type` 不跟链接：链接既不是目录也不是普通文件，跳过。
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata().map_or(0, |meta| meta.len());
            }
        }
    }
    Some(total)
}

/// 包文件夹里带的技能、脚本（施工 X-13 上）、斜杠命令模板（施工 X-14 上补，`slash`）：清单里写不出它们，照文件夹找。脚本装上就是这台机器上跑得起来的工具，同 MCP
/// 服务，装之前给人看名字。读不成的另列（施工 X-13 补）：包文件夹里的相对路径（`/` 分隔）和为什么。碰磁盘。
struct Content {
    skills: Vec<String>,
    scripts: Vec<String>,
    slash: Vec<String>,
    /// 带的人格、预设的编号（施工 P-11 下）。
    personas: Vec<String>,
    presets: Vec<String>,
    skipped: Vec<Value>,
}

impl Content {
    fn of(dir: &Path) -> Content {
        let (skills, broken_skills) = miyu_store::skills::carried(dir);
        let (scripts, broken_scripts) = miyu_store::scripts::carried(dir);
        let (slash, broken_slash) = miyu_store::slash::carried(dir);
        let skipped = broken_skills
            .iter()
            .chain(&broken_scripts)
            .chain(&broken_slash)
            .map(|one| {
                let file = one.path.strip_prefix(dir).unwrap_or(&one.path);
                let file = file
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                json!({"file": file, "why": one.why})
            })
            .collect();
        Content {
            skills,
            scripts,
            slash,
            personas: miyu_store::personas::carried(dir),
            presets: miyu_store::presets::carried(dir),
            skipped,
        }
    }
}

/// 一份清单带了什么：编号、版本（写了的）、程序（`ui`、`process`、`worker`、`builtin`，只带吉祥物的没有）、子命令名、后台页、
/// 吉祥物、接哪个平台、几个配置项、技能、脚本、读不成的技能和脚本（`content`）、声明的能力（照连接的语言说，`extension.status` 那一格的
/// 样子）。没有的不写。
fn carries(
    core: &Core,
    peer: Peer,
    id: &str,
    manifest: &Manifest,
    content: &Content,
) -> Map<String, Value> {
    let mut answer = Map::new();
    answer.insert("package".into(), json!(id));
    if let Some(version) = &manifest.version {
        answer.insert("version".into(), json!(version));
    }
    if !matches!(
        manifest.kind,
        PackageKind::Mascot | PackageKind::Theme | PackageKind::Bundle
    ) {
        answer.insert("program".into(), json!(manifest.kind.as_str()));
    }
    // 带的 MCP 服务（施工 X-2）：装上以后会在这台机器上跑它们的命令，装之前给人看名字。
    if !manifest.mcp.is_empty() {
        let names: Vec<&str> = manifest
            .mcp
            .iter()
            .map(|server| server.name.as_str())
            .collect();
        answer.insert("mcp".into(), json!(names));
    }
    // 提供的模型驱动（施工 MD-1，设计 66 第十节第 1 条）：用它的模型时发给模型的全部对话都经过它，装之前给人看它声明了哪几个。
    if !manifest.drivers.is_empty() {
        let names: Vec<&str> = manifest
            .drivers
            .iter()
            .map(|driver| driver.name.as_str())
            .collect();
        answer.insert("drivers".into(), json!(names));
    }
    // 第一个子命令照旧放在 `command`，几个都在 `commands`（施工 F-13）。
    if let Some(command) = manifest.command() {
        answer.insert("command".into(), json!(command.name));
        let names: Vec<&str> = manifest
            .commands
            .iter()
            .map(|one| one.name.as_str())
            .collect();
        answer.insert("commands".into(), json!(names));
    }
    if manifest.page.is_some() {
        answer.insert("page".into(), json!(true));
    }
    if manifest.mascot.is_some() {
        answer.insert("mascot".into(), json!(true));
    }
    if manifest.theme.is_some() {
        answer.insert("theme".into(), json!(true));
    }
    if let Some(connection) = &manifest.connection {
        // 接的平台（施工 IM-3 起能写几个）：`connection` 照旧是第一个；接几个的另带 `platforms`，是全部。
        answer.insert("connection".into(), json!(connection.platforms.first()));
        if connection.platforms.len() > 1 {
            answer.insert("platforms".into(), json!(connection.platforms));
        }
    }
    if !manifest.settings.is_empty() {
        answer.insert("settings".into(), json!(manifest.settings.len()));
    }
    if !content.skills.is_empty() {
        answer.insert("skills".into(), json!(content.skills));
    }
    if !content.scripts.is_empty() {
        answer.insert("scripts".into(), json!(content.scripts));
    }
    // 斜杠命令：模板、扩展声明的（施工 X-14 下）、整页的（施工 X-15 上）一起列，照名字排。
    let mut slash: Vec<&str> = content
        .slash
        .iter()
        .map(String::as_str)
        .chain(manifest.slash_names())
        .collect();
    slash.sort_unstable();
    if !slash.is_empty() {
        answer.insert("slash".into(), json!(slash));
    }
    if !content.personas.is_empty() {
        answer.insert("personas".into(), json!(content.personas));
    }
    if !content.presets.is_empty() {
        answer.insert("presets".into(), json!(content.presets));
    }
    if !content.skipped.is_empty() {
        answer.insert("skipped".into(), json!(content.skipped));
    }
    if let Some(listed) = crate::extensions::described(core, manifest, peer.language) {
        answer.insert("capabilities".into(), listed);
    }
    answer
}

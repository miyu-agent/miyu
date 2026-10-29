//! 装、卸之前看一眼（施工 F-8 下补，设计 `31-软件包.md` 第一节、定了的 B）：`package.install`、`package.remove` 带
//! `preview: true` 的，照真做之前查的那几样查一遍（写错的、撞了的、必需的照样拒），交回要做什么，什么都不动。命令行照它印一份、
//! 问「继续？[Y/n]」（`cli/pkg.md`）。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::packages::local;
use miyu_store::packages::{Layer, Packages};

use super::folder::source;
use super::manage::{clear_of_conflicts, removable, setting_keys, shipped_clear_of_conflicts};
use super::packages;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 从包文件夹 `path` 装：编号、版本、换下哪一份（同编号的）、带了什么、要什么能力、几个文件多大。
pub(crate) async fn installing(
    core: &Arc<Core>,
    peer: Peer,
    path: &Path,
) -> Result<Value, Refusal> {
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
    Ok(Value::Object(answer))
}

/// 把卸掉的出厂的包装回来：编号、版本、带了什么，`restores: true`。没卸过的 `unknown_package`。
pub(crate) async fn restoring(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    if !packages(core).removed().iter().any(|removed| removed == id) {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    shipped_clear_of_conflicts((core, peer), id).await?;
    let shipped = Packages::shipped(&core.resources).read();
    let one = shipped
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let manifest = one.read.as_ref().map_err(|_| Refusal::UNKNOWN_PACKAGE)?;
    let mut answer = carries(core, peer, id, manifest, &Content::of(&one.files_dir()));
    answer.insert("restores".into(), json!(true));
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

/// 卸：编号、哪一层、版本；家目录里的几个文件多大；一并删掉的配置项（配置里写了的键）、有没有状态目录。
pub(crate) async fn removing(core: &Arc<Core>, id: &str) -> Result<Value, Refusal> {
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
    answer.insert("settings".into(), json!(settings));
    let state = packages(core).state_dir(id);
    let state = tokio::task::spawn_blocking(move || state.is_dir())
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    answer.insert("state".into(), json!(state));
    Ok(Value::Object(answer))
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

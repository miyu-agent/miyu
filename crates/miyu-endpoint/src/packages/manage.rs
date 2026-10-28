//! 装、卸（施工 F-5 上，设计 `30-插件框架.md` 第九节，`docs/blueprint/packages.md`「装卸」）：`package.install`、
//! `package.remove` 改系统那一层的（照请求分层在 `asked.rs`，自己那一层的在 `mine.rs`，施工 U23 下上）。只动系统那一层（磁盘上
//! 怎么做在 `miyu_store::packages::install`），做完照两层重读、当场换掉核心手里的清单（[`Core::reload_packages`]）：
//! `package.list`、预设的功能、开着的会话下一个回合都照新的。一次只做一件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_config::package;
use miyu_policy::preset::MEMORY;
use miyu_store::packages::conflicts::conflicts;
use miyu_store::packages::install::{self, Placed};
use miyu_store::packages::{Found, Issue, Layer, Packages};

use super::folder::source;
use super::{TARGET, compiled, listed, load, packages, sentence, settle};
use crate::Core;
use crate::config::methods::words;
use crate::extensions::processes_of;
use crate::hello::Peer;
use crate::refusal::Refusal;

impl Core {
    /// 照两层重读软件包清单，标没编进来的内置包、认配置项撞没撞，当场换掉核心手里的那一份（施工 F-5 上）。整份配置清单照
    /// 端口拼、配置服务换上（施工 F-5 补）；没设端口的（测试里造的核心）照手里的清单认撞没撞，配置清单不换。
    pub(crate) fn reload_packages(&self) {
        // 列表里变了的推出去（施工 F-8 三补）：前后各照英文算一份比。
        let before = super::changes::snapshot(self);
        let mut found = load(&self.resources, &self.root, &self.admin);
        if let Some(built_in) = &self.built_in {
            compiled(&mut found, built_in);
        }
        let items = match &self.builtins {
            Some(builtins) => Some(builtins.settings(&mut found)),
            None => {
                let items = self.config().items().to_vec();
                let _ = settle(&mut found, &items);
                None
            }
        };
        self.rebuild_builtins(&found);
        *self
            .packages
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = std::sync::Arc::new(found);
        // 每人那几份照新的系统那一份重造（施工 U23 上下）：用到时再造。
        self.visible_packages.forget();
        // 脚本照新的清单重找、重登记（施工 X-12 中）。
        self.follow_scripts();
        if let Some(items) = items {
            crate::config::refit::refit(self, items);
        }
        super::changes::announce(self, &before, &super::changes::snapshot(self));
        // 给别的包带的内容照新的清单查一遍（施工 X-22）。
        self.content_rereads.bump();
    }

    /// 装卸时照清单从 `before` 换到 `now`（施工 F-5 下、补、再补）：人格记忆装没装照 `now` 设（`Memory::set_installed`，施工
    /// R-10：开着的会话照它交不交摘要、抽不抽），扩展进程跟着停下、拉起（[`Core::follow_packages`]），本机的向量模型照 `now`
    /// 换（`Vectors::replace_local`，一样的不动，旧的小程序退完才返回）。卸包、升级在动文件以前照去掉它的清单换一遍：Windows
    /// 上开着的文件删不掉、挪不走。
    async fn switch_packages(self: &Arc<Self>, before: &[&Found], now: &[&Found]) {
        let memory = now
            .iter()
            .any(|one| super::is_installed(std::slice::from_ref(*one), MEMORY));
        self.memory.set_installed(memory);
        self.follow_packages(&processes_of(before), &processes_of(now))
            .await;
        self.follow_bundles(before, now).await;
        crate::knowledge::follow(&self.knowledge, now);
        if let (Some(builtins), Some(vectors)) = (&self.builtins, self.memory.vectors()) {
            vectors.replace_local(builtins.embed(now)).await;
        }
    }

    /// 照清单 `found` 换内置包的工具（施工 F-5 中）：新装上的换进去，卸掉的拿掉、记下随包卸掉了（用过它的会话调到时报「已
    /// 卸载」）。没设端口、不知道编进来了哪些的不动。换不成的记一行、照旧。
    fn rebuild_builtins(&self, found: &[Found]) {
        let (Some(builtins), Some(built_in)) = (&self.builtins, &self.built_in) else {
            return;
        };
        let groups = match builtins.tools(found) {
            Ok(groups) => groups,
            Err(error) => {
                tracing::warn!(target: TARGET, error = error.as_str(), "builtin tools not rebuilt");
                return;
            }
        };
        let current = self.tools.current();
        let moving = built_in.iter().any(|package| {
            let has = current.packages().any(|owner| owner == *package);
            let wanted = groups.iter().any(|(id, _)| id == package);
            has != wanted
        });
        if !moving {
            return;
        }
        let changed = self.tools.replace(|catalog| {
            let mut next = catalog.clone();
            for package in built_in {
                let has = catalog.packages().any(|owner| owner == *package);
                let wanted = groups.iter().find(|(id, _)| id == package);
                next = match (wanted, has) {
                    (Some((_, tools)), false) => next.placing(package, tools.clone())?,
                    (None, true) => next.removing(package),
                    _ => next,
                };
            }
            Ok::<_, miyu_tool::CatalogError>(next)
        });
        if let Err(error) = changed {
            tracing::warn!(target: TARGET, error = %error, "builtin tools not rebuilt");
        }
    }
}

impl Core {
    /// 登记的查询 `method` 这时答不答（施工 F-5 中）：属于哪个软件包的，那个包这时装着才答；不属于哪个包的照答。
    pub(crate) fn serves(&self, method: &str) -> bool {
        self.queries
            .package_of(method)
            .is_none_or(|package| super::is_installed(&self.system_packages(), package))
    }
}

/// 卸 `id` 之前查的（卸、看一眼共用）：编号合写法、装着、不是必需的。
pub(super) fn removable<'a>(found: &'a [Found], id: &str) -> Result<&'a Found, Refusal> {
    if !miyu_store::personas::valid(id) {
        return Err(Refusal::BAD_PARAMS);
    }
    let one = found
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    // 被别的装着的包依赖着的卸不掉，说清被谁（施工 F-11，照 pacman）。
    let by = super::depends::dependents(found, id);
    if !by.is_empty() {
        return Err(Refusal::package_depended(&by));
    }
    Ok(one)
}

/// 包 `one` 的配置项的真的键（`<编号>.<名字>`），照清单列的；读坏了的清单没有。
pub(super) fn setting_keys(one: &Found) -> Vec<String> {
    one.read
        .as_ref()
        .map(|manifest| {
            manifest
                .settings
                .iter()
                .map(|setting| format!("{}.{}", one.id, setting.name))
                .collect()
        })
        .unwrap_or_default()
}

/// 卸包 `id`（`package.remove`，施工 F-6 上起 `package.disable` 关出厂的内置包也走这里）。
pub(super) async fn remove_package(core: &Arc<Core>, id: String) -> Result<Value, Refusal> {
    let _one_at_a_time = core.packaging.lock().await;
    let found = core.system_packages();
    let one = removable(&found, &id)?;
    let home = home(core)?;
    let local_root = packages(core).local_root();
    let state = packages(core).state_dir(&id);
    // 卸就是清干净（施工 F-8 中下补，设计 31 定了的 H）：它的配置项照清单列的删，状态目录整个删，开关、批过的能力删（施工
    // F-8 五补）。
    let keys = setting_keys(one);
    // 它带的 MCP 服务的缓存、工作目录也清掉（施工 X-3 下）：它们不在包的状态目录里。
    let services: Vec<String> = one
        .read
        .as_ref()
        .map(|manifest| {
            manifest
                .mcp
                .iter()
                .map(|server| server.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let layer = one.layer;
    // 它带的人格、预设在系统区、家目录里有改动的，卸成了记一笔是它留下的（施工 P-11 下）。
    let noting = super::left::noting(core, &id, &one.files_dir());
    // 先停下用着它的、再删文件（施工 F-5 补）：Windows 上开着的文件删不掉。删不成的照原来的清单换回来。
    let all = refs(&found);
    let without = leaving(&all, &id);
    core.switch_packages(&all, &without).await;
    let removed = blocking(move || {
        let left = noting();
        let removed = match layer {
            Layer::Home => install::take_out(&home, &id).map(|()| {
                super::local::forget(&local_root, &id);
                id
            }),
            Layer::Shipped => install::mark_removed(&home, &id).map(|()| id),
        };
        if removed.is_ok() {
            left.keep();
        }
        removed
    })
    .await;
    let removed = match removed {
        Ok(removed) => removed,
        Err(refusal) => {
            core.switch_packages(&without, &all).await;
            return Err(refusal);
        }
    };
    core.reload_packages();
    let now = core.system_packages();
    core.switch_packages(&without, &refs(&now)).await;
    crate::config::forget::forget(core, &keys);
    // 开关的文件里它开没开、批过的能力（施工 F-8 五补）：装回来从头开、从头批。
    crate::extensions::forget_switch(core, &removed).await;
    // 它的脚本在各账号下的状态目录（施工 X-44）：同包的状态目录一起删。
    let (root, package) = (core.root.path().to_path_buf(), removed.clone());
    let cleared = blocking(move || {
        remove_dir_if_there(&state)?;
        miyu_store::scripts::forget_states(&root, &package)
    });
    if let Err(refusal) = cleared.await {
        tracing::warn!(target: TARGET, package = removed.as_str(), reason = refusal.reason, "package state not removed");
    }
    for service in &services {
        crate::mcp::forget(core, service);
    }
    // 它拿得到的 blob（施工 W-6 三补）：装回来从空的开始。
    core.blob_grants.forget(&removed);
    tracing::info!(target: TARGET, package = removed.as_str(), "package removed");
    Ok(json!({"package": removed, "removed": true}))
}

/// 从 `path` 这个包文件夹装（施工 F-8 上：一个文件夹就是一个包；写成文件夹里的 `package.toml` 也认）：读得成、不和出厂的撞、
/// 带的名字不和装着的别的包撞（施工 F-8 六补）、拷进去以后照两层重读还是好的，才算装上。编号是文件夹的名字。
pub(super) async fn from_path(
    core: &Arc<Core>,
    peer: Peer,
    path: &Path,
    file: Option<&Path>,
) -> Result<Value, Refusal> {
    let (folder, id, manifest) = source(core, peer, path)?;
    clear_of_conflicts((core, peer), &id, manifest, folder.clone()).await?;
    let words = words(core, peer.language)?;
    let places = packages(core);
    let home = home(core)?;
    let source = folder.clone();
    let target = id.clone();
    // 升级的先停下原来的那一个再换文件（施工 F-5 补）：Windows 上开着的文件挪不走。换不成的照原来的清单换回来。
    let before = core.system_packages();
    let all = refs(&before);
    let without = leaving(&all, &id);
    core.switch_packages(&all, &without).await;
    let placed = blocking(move || install::place(&home, &target, &source)).await;
    let placed: Placed = match placed {
        Ok(placed) => placed,
        Err(refusal) => {
            core.switch_packages(&without, &all).await;
            return Err(refusal);
        }
    };
    core.reload_packages();
    let now = core.system_packages();
    let mine = now.iter().find(|one| one.id == id);
    if let Some(Found {
        read: Err(Issue::Wrong(problem)),
        ..
    }) = mine
    {
        let refusal = invalid(&words, problem);
        if let Err(error) = placed.undo() {
            tracing::warn!(target: TARGET, package = id.as_str(), error = %error, "package install not undone");
        }
        core.reload_packages();
        let back = core.system_packages();
        core.switch_packages(&without, &refs(&back)).await;
        return Err(refusal);
    }
    placed.keep();
    // 它以前卸掉时留下的改动接着叠在它上面，那几笔划掉（施工 P-11 下）。
    let back = super::left::back(core, &id);
    if blocking(move || {
        back();
        Ok(())
    })
    .await
    .is_err()
    {
        tracing::warn!(target: TARGET, package = id.as_str(), "package leftovers not cleared");
    }
    // 本地库记下这一份（施工 F-8 中上）：记不成的记一行，装成了照算。
    let version = mine
        .and_then(|one| one.read.as_ref().ok())
        .and_then(|manifest| manifest.version.clone());
    let (record_places, record_id, from) = (
        places.clone(),
        id.clone(),
        file.unwrap_or(&folder).display().to_string(),
    );
    if let Err(refusal) =
        blocking(move || super::local::record(&record_places, &record_id, version, &from)).await
    {
        tracing::warn!(target: TARGET, package = id.as_str(), reason = refusal.reason, "package not recorded");
    }
    core.switch_packages(&without, &refs(&now)).await;
    tracing::info!(target: TARGET, package = id.as_str(), "package installed");
    let mine = now
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::INTERNAL)?;
    Ok(listed(
        core,
        mine,
        (&places, &now),
        &words,
        peer.language,
        false,
    ))
}

/// 把卸掉的出厂的包 `id` 装回来：删掉家目录里记的那一笔。
pub(super) async fn bring_back(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let places = packages(core);
    if !places.removed().iter().any(|removed| removed == id) {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    shipped_clear_of_conflicts((core, peer), id).await?;
    let home = home(core)?;
    let target = id.to_string();
    let before = core.system_packages();
    let back = super::left::back(core, id);
    blocking(move || {
        install::unmark_removed(&home, &target)?;
        back();
        Ok(())
    })
    .await?;
    core.reload_packages();
    let now = core.system_packages();
    core.switch_packages(&refs(&before), &refs(&now)).await;
    tracing::info!(target: TARGET, package = id, "package restored");
    let words = words(core, peer.language)?;
    let now = core.system_packages();
    let back = now
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    Ok(listed(
        core,
        back,
        (&places, &now),
        &words,
        peer.language,
        false,
    ))
}

/// 要装的包（编号 `id`、清单 `manifest`、文件夹 `dir`）和装着的别的包、核心内置的斜杠命令（施工 X-14 上补）重了名的
/// （施工 F-8 六补，`packages.md`「冲突」）：
/// `package_conflict`，什么都不动。读包文件夹在阻塞线程里。
pub(super) async fn clear_of_conflicts(
    (core, peer): (&Arc<Core>, Peer),
    id: &str,
    manifest: package::Manifest,
    dir: PathBuf,
) -> Result<(), Refusal> {
    super::may_manage::may_manage(peer.admin)?;
    let installed = core.system_packages();
    // 依赖的包没装的装不上（施工 F-11）：升级时它自己那一份不算。软件源做好以后再定要不要自动装。
    let others: Vec<&Found> = installed.iter().filter(|one| one.id != id).collect();
    let missing = super::depends::missing(&others, &manifest);
    if !missing.is_empty() {
        return Err(Refusal::depends_missing(&missing));
    }
    let reserved = core
        .commands
        .reserved(|package| super::is_installed(&installed, package));
    // 人格、预设还和出厂、系统区、家目录里有的比（施工 P-11 下）。
    let layers = super::left::layers(core, id);
    let id = id.to_string();
    let found = blocking(move || {
        Ok(conflicts(
            &id, &manifest, &dir, &installed, &reserved, &layers,
        ))
    })
    .await?;
    match found.is_empty() {
        true => Ok(()),
        false => Err(Refusal::package_conflict(&found)),
    }
}

/// 把卸掉的出厂的包 `id` 装回来之前也查（同 [`clear_of_conflicts`]）：卸掉以后家目录里装了同名的。
pub(super) async fn shipped_clear_of_conflicts(
    (core, peer): (&Arc<Core>, Peer),
    id: &str,
) -> Result<(), Refusal> {
    let shipped = Packages::shipped(&core.resources).read();
    let Some((manifest, dir)) = shipped
        .iter()
        .find(|one| one.id == id)
        .and_then(|one| Some((one.read.as_ref().ok()?.clone(), one.files_dir())))
    else {
        return Ok(());
    };
    clear_of_conflicts((core, peer), id, manifest, dir).await
}

/// 管理员家目录那一层。
fn home(core: &Core) -> Result<PathBuf, Refusal> {
    packages(core)
        .home_dir()
        .map(Path::to_path_buf)
        .ok_or(Refusal::INTERNAL)
}

/// 清单写错了、撞了：照连接的语言说一句。
pub(super) fn invalid(words: &miyu_store::human::Human, problem: &package::Problem) -> Refusal {
    let said = sentence(words, problem.code.as_str(), &problem.detail)
        .unwrap_or_else(|| problem.message.clone());
    Refusal::package_invalid(said, problem.line)
}

/// 在阻塞线程里动磁盘；动不了的记一行、算内部出错。
pub(super) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> std::io::Result<T> + Send + 'static,
) -> Result<T, Refusal> {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(done)) => Ok(done),
        Ok(Err(error)) => {
            tracing::warn!(target: TARGET, error = %error, "package files not changed");
            Err(Refusal::INTERNAL)
        }
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// 清单里的每一份。
fn refs(found: &[Found]) -> Vec<&Found> {
    found.iter().collect()
}

/// 去掉编号是 `id` 的那一份。
fn leaving<'a>(all: &[&'a Found], id: &str) -> Vec<&'a Found> {
    all.iter().copied().filter(|one| one.id != id).collect()
}

/// 删掉目录 `dir`；本来就没有的不算错。
fn remove_dir_if_there(dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

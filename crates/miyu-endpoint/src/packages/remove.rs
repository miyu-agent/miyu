//! 卸（施工 F-5 上；施工 PK-5补 从 `manage.rs` 挪出来，行数）：`package.remove` 卸装着的包，`package.disable` 关出厂的、删人格
//! 连包卸、她的工具卸的也走这里。卸就是清干净，只有给人改的两样照 pacman 默认留下、`purge` 的才删：配置目录 `etc/<编号>/`
//! （施工 PK-5）、`etc/config.toml` 里它的设置项（施工 PK-5补，留下的怎么认见 `config/left.rs`）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_store::packages::install;
use miyu_store::packages::settings_left;
use miyu_store::packages::{Found, Layer};

use super::manage::{blocking, home, leaving, refs};
use super::{TARGET, packages};
use crate::Core;
use crate::refusal::Refusal;

/// 卸 `id` 之前查的（卸、看一眼共用）：编号合写法、装着、不是必需的。
pub(crate) fn removable<'a>(found: &'a [Found], id: &str) -> Result<&'a Found, Refusal> {
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

/// 包 `one` 的配置项的真的键（`<前缀>.<名字>`，施工 PN-1），照清单列的；读坏了的清单没有。
pub(crate) fn setting_keys(one: &Found) -> Vec<String> {
    one.read
        .as_ref()
        .map(|manifest| {
            manifest
                .settings
                .iter()
                .map(|setting| format!("{}.{}", manifest.prefix_of(&one.id), setting.name))
                .collect()
        })
        .unwrap_or_default()
}

/// 卸包 `id`（`package.remove`，施工 F-6 上起 `package.disable` 关出厂的内置包也走这里）。配置目录 `etc/<编号>/`、配置里它的
/// 设置项照 pacman 默认留下（施工 PK-5、PK-5补），`purge` 的才一起删。
pub(crate) async fn remove_package(
    core: &Arc<Core>,
    id: String,
    purge: bool,
) -> Result<Value, Refusal> {
    let _one_at_a_time = core.packaging.lock().await;
    let found = core.system_packages();
    let one = removable(&found, &id)?;
    let home = home(core)?;
    let local_root = packages(core).local_root();
    let markers = packages(core).removed_markers();
    // 卸就是清干净（施工 F-8 中下补，设计 31 定了的 H；设计 55 第三节第 4 条）：各层它的 `<编号>/` 整个删，开关、批过的能力删
    // （施工 F-8 五补）。给人改的两样照 pacman 默认留下、重装同编号照旧用，`purge` 的才删（2026-11-05 项目主人定）：配置目录
    // `etc/<编号>/`（施工 PK-5）、配置里它的设置项（施工 PK-5补）。
    let config = core.root.package_config(&id);
    let places: Vec<PathBuf> = core
        .root
        .package_places(&id)
        .into_iter()
        .filter(|place| purge || *place != config)
        .collect();
    let keys = setting_keys(one);
    // 留下了设置项的你装的包记下前缀（施工 PK-5补）：卸了整个文件夹都没了，读配置时照它认出这几个键。出厂的、系统加装的不记：
    // 清单还在。下面一层还有同编号的（被它盖住的）也不记：卸了那一份顶上来，照它的清单认。
    let mark =
        (!purge && one.layer == Layer::Home && !super::settings_left::lower_has(&found, &id))
            .then(|| {
                let prefix = one.read.as_ref().ok()?.prefix_of(&id).to_string();
                (!crate::config::forget::present(core, &keys).is_empty()).then_some(prefix)
            })
            .flatten();
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
    // 它带的人格、预设在你那一层有改动的，卸成了记一笔是它留下的（施工 P-11 下）。
    let noting = super::left::noting(core, &id, &one.files_dir());
    // 先停下用着它的、再删文件（施工 F-5 补）：Windows 上开着的文件删不掉。删不成的照原来的清单换回来。
    let all = refs(&found);
    let without = leaving(&all, &id);
    core.switch_packages(&all, &without).await;
    let root = core.root.clone();
    let removed = blocking(move || {
        let left = noting();
        let removed = match layer {
            Layer::Home => install::take_out(&home, &id).map(|()| {
                super::local::forget(&local_root, &id);
                id
            }),
            // 出厂的、系统加装的（施工 SD-1）：文件不动，记一笔不再加载；系统加装的文件归系统的包管理器。
            Layer::Shipped | Layer::Distro => install::mark_removed(&markers, &id).map(|()| id),
        };
        if let Ok(id) = &removed {
            left.keep();
            settled(&root, id, mark.as_deref());
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
    if purge {
        crate::config::forget::forget(core, &keys);
    }
    // 开关的文件里它开没开、批过的能力（施工 F-8 五补）：装回来从头开、从头批。
    crate::extensions::forget_switch(core, &removed).await;
    // 它的脚本的状态和缓存（施工 X-44）、它交过的「给」的那一份（`var/cache/provide/<编号>.json`）：同各层的目录一起删。
    let (root, package) = (core.root.clone(), removed.clone());
    let cleared = blocking(move || {
        for place in &places {
            remove_dir_if_there(place)?;
        }
        if !purge {
            drop_if_empty(&config);
        }
        remove_file_if_there(&root.provide_cache(&package))?;
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
    // 系统加装的说一声：文件还在，要删用系统的包管理器（施工 SD-1，命令行照 `layer` 说）。
    Ok(json!({"package": removed, "removed": true, "layer": layer.as_str()}))
}

/// 卸成了以后照 `mark` 记下（留了设置项的前缀）或者删掉（施工 PK-5补）包 `id` 留下的设置项的记号。记不成、删不掉的记一行，
/// 卸照样算成：没记上的那几个键照不认识报。
fn settled(root: &miyu_store::root::DataRoot, id: &str, mark: Option<&str>) {
    let done = match mark {
        Some(prefix) => settings_left::mark(root, id, prefix),
        None => settings_left::unmark(root, id),
    };
    if let Err(error) = done {
        tracing::warn!(target: TARGET, package = id, error = %error, "package leftover settings not noted");
    }
}

/// 删掉目录 `dir`；本来就没有的不算错。是链接的只删链接（`remove_dir_all` 不跟链接）。
pub(super) fn remove_dir_if_there(dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// 留下的配置目录 `dir` 是空的（拉起扩展前各处都建好，没写过东西的也在）就删掉，不留空目录（施工 PK-5）；是链接的、不空的、
/// 删不掉的不动。
fn drop_if_empty(dir: &Path) {
    if std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        drop(std::fs::remove_dir(dir));
    }
}

/// 删掉文件 `file`；本来就没有的不算错。
fn remove_file_if_there(file: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(file) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

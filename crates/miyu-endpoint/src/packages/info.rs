//! `package.info`（施工 F-8 中上起，IN-2 照设计 `49-装包照pacman.md` 第一节、第三节第 2 条补）：装着的照本地（pacman 的
//! `-Qi`），没装的照软件源（`-Si`，[`super::source::look`]）。`source` 写 `true` 的只查软件源，`false` 的只查装着的，不写的先查
//! 装着的、没装的再查软件源。不写 `package`、写 `path` 的看本机一个包文件夹、包文件（施工 IN-4，`file_info.rs`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use super::{TARGET, depends, packages};
use crate::Core;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;

/// `package.info` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InfoParams {
    #[serde(default)]
    package: Option<String>,
    /// 本机一个包文件夹、包文件的绝对路径（施工 IN-4）：和 `package` 二选一，不收 `source`。
    #[serde(default)]
    path: Option<String>,
    /// 查哪一处（施工 IN-2）：`true` 只查软件源，`false` 只查装着的，不写的先查装着的。
    #[serde(default)]
    source: Option<bool>,
}

/// `package.info`：照 `source` 查。不写 `source` 的，装着的没有、软件源里也找不到（拿不成的也算）的一律 `unknown_package`；
/// 只查软件源的照从软件源装的拒。`package`、`path` 都写、都没写的，带 `path` 又写了 `source` 的 `bad_params`。
pub(crate) async fn info(
    core: &Arc<Core>,
    peer: Peer,
    params: InfoParams,
) -> Result<Value, Refusal> {
    match (params.package, params.path, params.source) {
        (Some(id), None, source) => by_id(core, peer, &id, source).await,
        (None, Some(path), None) => super::file_info::file(core, peer, &path).await,
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// 照编号 `id` 查，`source` 同参数那一格。
async fn by_id(
    core: &Arc<Core>,
    peer: Peer,
    id: &str,
    source: Option<bool>,
) -> Result<Value, Refusal> {
    match source {
        Some(true) => super::source::look::look(core, peer, id).await,
        Some(false) => installed(core, peer, id).await,
        None => match installed(core, peer, id).await {
            Err(refusal) if refusal == Refusal::UNKNOWN_PACKAGE => {
                super::source::look::look(core, peer, id)
                    .await
                    .map_err(|refusal| {
                        tracing::info!(target: TARGET, package = id, reason = refusal.reason, "package not found in sources");
                        Refusal::UNKNOWN_PACKAGE
                    })
            }
            other => other,
        },
    }
}

/// 装着的那一份：[`super::local::info`] 的几格，另有清单写的名字、说明、依赖、推荐，带了什么（`provides`，同装之前看一眼的
/// 「包含」）、要的能力（`capabilities`），装着的包里依赖它的（`required_by`，照编号），从软件源装的来源仓库（`origin`）。没有的
/// 不写。
async fn installed(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let mut answer = super::local::info(core, peer.for_owner, id).await?;
    let now = core.system_packages();
    if let Some(found) = now.iter().find(|one| one.id == id)
        && let Ok(manifest) = &found.read
    {
        for (key, value) in [
            ("name", pick(&manifest.name, peer.language)),
            ("summary", pick(&manifest.summary, peer.language)),
        ] {
            if let Some(value) = value {
                answer[key] = json!(value);
            }
        }
        for (key, ids) in [
            ("depends", &manifest.depends),
            ("recommends", &manifest.recommends),
        ] {
            if !ids.is_empty() {
                answer[key] = json!(ids);
            }
        }
        let (provides, capabilities) =
            super::preview::provided(core, peer, (id, manifest), &found.files_dir());
        if provides.as_object().is_some_and(|map| !map.is_empty()) {
            answer["provides"] = provides;
        }
        if let Some(capabilities) = capabilities {
            answer["capabilities"] = capabilities;
        }
    }
    let required_by = depends::dependents(&now, id);
    if !required_by.is_empty() {
        answer["required_by"] = json!(required_by);
    }
    let root = packages(core).local_root();
    let target = id.to_string();
    let origin =
        tokio::task::spawn_blocking(move || miyu_store::packages::local::origin(&root, &target))
            .await
            .map_err(|_| Refusal::INTERNAL)?;
    if let Some(origin) = origin {
        answer["origin"] = json!(origin);
    }
    Ok(answer)
}

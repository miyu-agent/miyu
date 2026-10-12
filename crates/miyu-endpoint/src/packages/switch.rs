//! 列表上的开关（施工 F-6 上，`docs/blueprint/package-pages.md`「开关」）：头只发 `package.enable`、`package.disable`，怎么做照
//! 种类定。扩展开关进程（同 `extension.enable`、`extension.disable`）；出厂的、不是必需的内置包关是卸掉（在家目录记一笔）、
//! 开是装回来；必需的、界面、小程序没有开关。`package.restart`（施工 X-4）重启扩展的进程和带的 MCP 服务、`bundle` 带的服务。
//! 回应同 `package.list` 的一项。

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::packages::Layer;

use super::{listed, manage, packages};
use crate::Core;
use crate::config::methods::words;
use crate::extensions::methods::{restart_process, turn_off, turn_on};
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `package.enable` 的参数：`approve` 同 `extension.enable`，批扩展要的能力。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EnableParams {
    package: String,
    #[serde(default)]
    approve: Option<Vec<String>>,
}

/// `package.disable`、`package.restart` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DisableParams {
    package: String,
}

/// 开关怎么做。
enum Switch {
    /// 扩展：开关进程。
    Extension,
    /// 出厂的、不是必需的内置包：卸掉、装回来。
    Shipped,
    /// `bundle`：开关它带的 MCP 服务（施工 X-3 上）。
    Bundle,
}

/// 包 `id` 现在的清单、是不是卸掉了的出厂包；没有的 `unknown_package`。
fn find(core: &Core, id: &str) -> Result<(Manifest, Layer, PathBuf, bool), Refusal> {
    if let Some(found) = core.packages().iter().find(|found| found.id == id) {
        let manifest = found
            .read
            .as_ref()
            .map_err(|_| Refusal::UNKNOWN_PACKAGE)?
            .clone();
        return Ok((manifest, found.layer, found.files_dir(), false));
    }
    let removed = packages(core).read_removed();
    let found = removed
        .iter()
        .find(|found| found.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let manifest = found
        .read
        .as_ref()
        .map_err(|_| Refusal::UNKNOWN_PACKAGE)?
        .clone();
    Ok((manifest, found.layer, found.files_dir(), true))
}

/// 这个包的开关怎么做：必需的 `package_required`，没有开关的 `not_switchable`。
fn switch(manifest: &Manifest, layer: Layer) -> Result<Switch, Refusal> {
    if manifest.required {
        return Err(Refusal::PACKAGE_REQUIRED);
    }
    match manifest.kind {
        PackageKind::Process => Ok(Switch::Extension),
        PackageKind::Builtin if layer == Layer::Shipped => Ok(Switch::Shipped),
        // 只带技能、脚本的没有开关（施工 X-13 上）。
        PackageKind::Bundle if crate::packages::status::runs(manifest) => Ok(Switch::Bundle),
        PackageKind::Builtin
        | PackageKind::Ui
        | PackageKind::Worker
        | PackageKind::Mascot
        | PackageKind::Bundle => Err(Refusal::NOT_SWITCHABLE),
    }
}

/// `package.enable`。卸掉了的出厂包先装回来；扩展再照 `approve` 打开。
pub(crate) async fn enable(
    core: &Arc<Core>,
    peer: Peer,
    params: EnableParams,
) -> Result<Value, Refusal> {
    let (manifest, layer, dir, removed) = find(core, &params.package)?;
    let how = switch(&manifest, layer)?;
    if crate::packages::status::program_missing(&dir, &manifest) {
        return Err(Refusal::PROGRAM_MISSING);
    }
    if removed {
        manage::bring_back(core, peer, &params.package).await?;
    }
    match how {
        Switch::Extension => {
            turn_on(core, peer, &params.package, params.approve.as_deref()).await?;
        }
        Switch::Bundle => crate::mcp::turn_on(core, &params.package, &manifest).await?,
        Switch::Shipped => {}
    }
    item(core, peer, &params.package)
}

/// `package.disable`。扩展关掉进程；出厂的内置包卸掉。
pub(crate) async fn disable(
    core: &Arc<Core>,
    peer: Peer,
    params: DisableParams,
) -> Result<Value, Refusal> {
    let (manifest, layer, _, removed) = find(core, &params.package)?;
    let how = switch(&manifest, layer)?;
    if !removed {
        match how {
            Switch::Extension => {
                turn_off(core, peer, &params.package).await?;
            }
            Switch::Shipped => {
                manage::remove_package(core, params.package.clone()).await?;
            }
            Switch::Bundle => crate::mcp::turn_off(core, &params.package, &manifest).await?,
        }
    }
    item(core, peer, &params.package)
}

/// `package.restart`（施工 X-4）：扩展同 `extension.restart`（关着的、要批的能力还有没批的照它拒），再重启它带的 MCP 服务；
/// `bundle` 重启它带的服务。关着的 `package_off`；别的种类没有能重启的，`not_restartable`。
pub(crate) async fn restart(
    core: &Arc<Core>,
    peer: Peer,
    params: DisableParams,
) -> Result<Value, Refusal> {
    let (manifest, _, _, removed) = find(core, &params.package)?;
    let id = params.package.as_str();
    if !crate::packages::status::runs(&manifest) {
        return Err(Refusal::NOT_RESTARTABLE);
    }
    if removed || !crate::extensions::switched_on(core, id, &manifest) {
        return Err(Refusal::PACKAGE_OFF);
    }
    if manifest.kind == PackageKind::Process {
        restart_process(core, peer, id).await?;
    }
    core.restart_services(id, &manifest).await;
    item(core, peer, id)
}

/// 包 `id` 这时在 `package.list` 里的那一项（卸掉了的出厂包带 `removed: true`）。
fn item(core: &Core, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let places = packages(core);
    if let Some(found) = core.packages().iter().find(|found| found.id == id) {
        return Ok(listed(core, found, &places, &words, peer.language, false));
    }
    let removed = places.read_removed();
    let found = removed
        .iter()
        .find(|found| found.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    Ok(listed(core, found, &places, &words, peer.language, true))
}

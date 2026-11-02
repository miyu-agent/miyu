//! 删人格（施工 P-3 下，`docs/blueprint/personas.md`「改」；施工 PE-1 从 `write.rs` 挪出来）：你那一层有的，整个目录挪进回收处
//! （`miyu_store::trash::personas`），回到下面那一层。你那一层没有、只有出厂包带的，那个出厂包又只带这一个人格的（施工 PE-1，
//! 2026-10-31 项目主人定：出厂的软件工程师能删），删它就是卸这个包，同 `package.remove`（`etc/removed/<编号>`），
//! `package.install` 写包编号装回来。装上的包带的、包里还有别的的照旧删不掉。
//!
//! 记忆（设计 55 第三节第 6 条）：删的是你那一层、删完三层里都没有这个编号了的，记忆跟着进回收处
//! （`trash::personas::take_memory`），登记里开着的那一份放下；删的只是盖在下面一层上的、连包卸掉的（装回来还是它），记忆不动。

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value as Json, json};

use miyu_config::package::PackageKind;
use miyu_store::layers::valid;
use miyu_store::packages::{Found as Package, Layer as PackageLayer};
use miyu_store::personas::{Personas, carried};
use miyu_store::recall::Room;
use miyu_store::trash;

use super::{TARGET, personas};
use crate::Core;
use crate::refusal::Refusal;
use crate::sessions::now;

/// `persona.delete` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeleteParams {
    persona: String,
}

/// 删的是哪一样。
enum Deleting {
    /// 你那一层的挪进了回收处。
    Home,
    /// 要卸的出厂包的编号。
    Package(String),
}

/// `persona.delete`：交回 `{"remains"}`：删完下面几层还有没有。你那一层没有、也没有能连包卸掉的出厂包的 `nothing_to_delete`；
/// 卸包被拒的照 `package.remove` 的拒绝（被别的包依赖着的 `package_depended`）。
pub(crate) async fn delete(core: &Arc<Core>, params: DeleteParams) -> Result<Json, Refusal> {
    if !valid(&params.persona) {
        return Err(Refusal::BAD_PARAMS);
    }
    let layers = personas(core);
    let packages = core.packages_of();
    let (root, at) = (core.root.clone(), now());
    let logs = std::sync::Arc::clone(&core.memory.logs);
    let id = params.persona;
    let deleting = tokio::task::spawn_blocking({
        let id = id.clone();
        move || {
            let dir = layers.home_dir(&id);
            if !dir.is_dir() {
                return sole_package(&packages, &layers, &id)
                    .map(Deleting::Package)
                    .ok_or(Refusal::NOTHING_TO_DELETE);
            }
            if let Err(error) = trash::personas::discard(&root, &dir, &id, at) {
                tracing::warn!(target: TARGET, kind = ?error.kind(), "persona not deleted");
                return Err(Refusal::INTERNAL);
            }
            if !layers.exists(&id) {
                logs.forget(&Room::Persona {
                    persona: id.clone(),
                });
                // 挪不成的记忆留在原处，人格照删：同编号再建时接着用。
                if let Err(error) = trash::personas::take_memory(&root, &id, at) {
                    tracing::warn!(target: TARGET, kind = ?error.kind(), "persona memory kept in place");
                }
            }
            Ok(Deleting::Home)
        }
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    if let Deleting::Package(package) = deleting {
        crate::packages::manage::remove_package(core, package).await?;
    }
    // 人起的编号不进运行日志（LG-1）。
    tracing::info!(target: TARGET, "persona deleted");
    let layers = personas(core);
    let remains = tokio::task::spawn_blocking(move || layers.exists(&id))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    Ok(json!({"remains": remains}))
}

/// 人格 `id` 能连包卸掉的那个出厂包的编号：出厂资源、你那一层都没有它，认的那一份（出厂的包在前、同一层照编号，同
/// [`crate::packages::folders`]）在出厂包里，那个包只带这一个人格（[`only_the_persona`]）。别的没有。读盘，在阻塞线程里调。
pub(crate) fn sole_package(packages: &[Package], personas: &Personas, id: &str) -> Option<String> {
    if !personas.layers_with(id).is_empty() {
        return None;
    }
    let mut read: Vec<&Package> = packages.iter().filter(|one| one.read.is_ok()).collect();
    read.sort_by(|a, b| (a.layer, &a.id).cmp(&(b.layer, &b.id)));
    let one = read
        .into_iter()
        .find(|one| one.files_dir().join("personas").join(id).is_dir())?;
    (one.layer == PackageLayer::Shipped && only_the_persona(one, id)).then(|| one.id.clone())
}

/// 包 `one` 只带人格 `id`：清单读得成、不带程序（`bundle`），没有配置项、MCP 服务、吉祥物、给别的包带的内容；包文件夹里只有
/// 清单、图标和 `personas/`，`personas/` 里只有 `id` 这一个。技能、脚本、预设住在包文件夹里，有了就不算。
pub(crate) fn only_the_persona(one: &Package, id: &str) -> bool {
    let Ok(manifest) = &one.read else {
        return false;
    };
    let bare = manifest.kind == PackageKind::Bundle
        && manifest.settings.is_empty()
        && manifest.mcp.is_empty()
        && manifest.mascot.is_none()
        && manifest.content.is_empty();
    let dir = one.files_dir();
    let logo = manifest
        .logo
        .as_deref()
        .and_then(|logo| Path::new(logo).components().next())
        .map(|first| first.as_os_str().to_os_string());
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return false;
    };
    let only_these = entries.flatten().all(|entry| {
        let name = entry.file_name();
        name == "package.toml" || name == "personas" || logo.as_ref() == Some(&name)
    });
    bare && only_these && carried(&dir) == [id]
}

#[cfg(test)]
mod tests;

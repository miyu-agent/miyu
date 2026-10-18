//! 升级（施工 G-5，设计 `37-软件源.md` 第四节第 6、7 条，照 pacman 的 `-Su`）：`package.upgrades` 盘点、`package.upgrade` 换。
//!
//! - 只碰从软件源装的：本地库记了来源仓库的（施工 G-4 上）；从本机文件夹、包文件装的不碰（照 pacman 的 `-Qm`）。
//! - 索引里那一版和装着的不一样就升（不比大小：作者撤回新版也跟着回去）；来源仓库和装的时候不一样的不升，报一句；
//!   `packages.hold` 里的跳过；软件源里没有了的、被拉黑的照用，报一句（第 5 条，装着的人自己定卸不卸）。
//! - 换就是照从软件源装那一条重装这一个（[`super::fetch`]）：缺的新依赖一起装，设置、状态、开关都留着；新版多要的能力照旧
//!   要人批了才开（扩展的批准记在 `system/extensions.json`，核心照它起）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_source::index::Index;

use super::super::packages;
use super::fetch::{self, SourceParams};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::reverse::Peer as Reverse;
use crate::settings::SourceSettings;

/// 装着的一个：编号、清单写的版本、来源仓库（从软件源装的才有）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Installed {
    id: String,
    version: Option<String>,
    origin: Option<String>,
}

/// 盘点的结果。
#[derive(Debug, Default, PartialEq, Eq)]
struct Survey {
    /// 能升的：编号、装着的版本、索引里的版本。
    upgrades: Vec<(String, Option<String>, String)>,
    /// 来源仓库变了的：编号、装的时候的、索引里的。
    moved: Vec<(String, String, String)>,
    /// 软件源里没有了的。
    gone: Vec<String>,
    /// 被拉黑的：编号、理由。
    blocked: Vec<(String, String)>,
    /// `packages.hold` 跳过的。
    held: Vec<String>,
}

/// `package.upgrade` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpgradeParams {
    /// 只升这几个；不写的是能升的全部。
    #[serde(default)]
    packages: Option<Vec<String>>,
    /// 只看一眼。
    #[serde(default)]
    preview: bool,
}

impl UpgradeParams {
    /// 只升 `only`（没有的是全部），`preview` 的只看一眼（她的工具经端口升，施工 G-6）。
    pub(crate) fn of(only: Option<String>, preview: bool) -> UpgradeParams {
        UpgradeParams {
            packages: only.map(|one| vec![one]),
            preview,
        }
    }
}

/// `package.upgrades`：回 `{"upgrades", "moved", "gone", "blocked", "held", "generated", "stale"?}`。
pub(crate) async fn upgrades(core: &Arc<Core>) -> Result<Value, Refusal> {
    let synced = super::sync::sync(core, false).await?;
    let survey = survey_core(core, &synced.index).await?;
    let mut answer = shown(&survey);
    answer["generated"] = json!(synced.index.generated);
    if let Some(stale) = synced.stale {
        answer["stale"] = Value::String(stale);
    }
    Ok(answer)
}

/// `package.upgrade`：一个一个换；看一眼的回 `{"upgrades", "download"}`，真换的回 `{"upgraded"}`（都是
/// `[{"package", "from", "to"}]`）。
pub(crate) async fn upgrade(
    core: &Arc<Core>,
    peer: Peer,
    caller: Option<&Reverse>,
    params: UpgradeParams,
) -> Result<Value, Refusal> {
    let synced = super::sync::sync(core, false).await?;
    let survey = survey_core(core, &synced.index).await?;
    let wanted: Vec<&(String, Option<String>, String)> = survey
        .upgrades
        .iter()
        .filter(|(id, ..)| {
            params
                .packages
                .as_ref()
                .is_none_or(|only| only.contains(id))
        })
        .collect();
    let rows: Vec<Value> = wanted
        .iter()
        .map(|(id, from, to)| json!({"package": id, "from": from, "to": to}))
        .collect();
    if params.preview {
        let download: u64 = wanted
            .iter()
            .filter_map(|(id, ..)| synced.index.entry(id))
            .filter_map(fetch::pick)
            .map(|file| file.size)
            .sum();
        return Ok(json!({"upgrades": rows, "download": download}));
    }
    for (id, ..) in &wanted {
        fetch::install(core, peer, caller, SourceParams::of(id)).await?;
    }
    Ok(json!({"upgraded": rows}))
}

/// 照核心手里的包、本地库、配置盘点。
async fn survey_core(core: &Arc<Core>, index: &Index) -> Result<Survey, Refusal> {
    let places = packages(core);
    let found: Vec<(String, Option<String>)> = core
        .packages()
        .iter()
        .map(|found| {
            let version = found.read.as_ref().ok().and_then(|m| m.version.clone());
            (found.id.clone(), version)
        })
        .collect();
    let root = places.local_root();
    let installed = tokio::task::spawn_blocking(move || {
        found
            .into_iter()
            .map(|(id, version)| {
                let origin =
                    miyu_store::packages::local::read(&root, &id).and_then(|(desc, _)| desc.origin);
                Installed {
                    id,
                    version,
                    origin,
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let hold = SourceSettings::from(&core.config().resolved().values())
        .hold
        .unwrap_or_default();
    Ok(survey(&installed, index, &hold))
}

/// 盘点：只看记了来源仓库的。
fn survey(installed: &[Installed], index: &Index, hold: &[String]) -> Survey {
    let mut survey = Survey::default();
    for one in installed {
        let Some(origin) = &one.origin else {
            continue;
        };
        if let Some(blocked) = index.blocked(&one.id, origin) {
            survey
                .blocked
                .push((one.id.clone(), blocked.reason.clone()));
            continue;
        }
        let Some(entry) = index.entry(&one.id) else {
            survey.gone.push(one.id.clone());
            continue;
        };
        if entry.origin != *origin {
            survey
                .moved
                .push((one.id.clone(), origin.clone(), entry.origin.clone()));
        } else if one.version.as_deref() == Some(entry.version.as_str()) {
        } else if hold.contains(&one.id) {
            survey.held.push(one.id.clone());
        } else {
            survey
                .upgrades
                .push((one.id.clone(), one.version.clone(), entry.version.clone()));
        }
    }
    survey
}

/// 盘点写成回应。
fn shown(survey: &Survey) -> Value {
    json!({
        "upgrades": survey.upgrades.iter().map(|(id, from, to)| json!({"package": id, "from": from, "to": to})).collect::<Vec<_>>(),
        "moved": survey.moved.iter().map(|(id, was, now)| json!({"package": id, "was": was, "now": now})).collect::<Vec<_>>(),
        "gone": survey.gone,
        "blocked": survey.blocked.iter().map(|(id, reason)| json!({"package": id, "reason": reason})).collect::<Vec<_>>(),
        "held": survey.held,
    })
}

#[cfg(test)]
mod tests;

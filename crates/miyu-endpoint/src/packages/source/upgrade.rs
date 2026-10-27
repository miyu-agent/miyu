//! 升级（施工 G-5，设计 `37-软件源.md` 第四节第 6、7 条，照 pacman 的 `-Su`）：`package.upgrades` 盘点、`package.upgrade` 换。
//!
//! - 只碰从软件源装的：本地库记了来源仓库的（施工 G-4 上）；从本机文件夹、包文件装的不碰（照 pacman 的 `-Qm`）。
//! - 索引里那一版和装着的不一样就升（不比大小：作者撤回新版也跟着回去）；来源仓库和装的时候不一样的不升，报一句；
//!   `packages.hold` 里的跳过；软件源里没有了的、被拉黑的照用，报一句（第 5 条，装着的人自己定卸不卸）。
//! - 换就是照从软件源装那一条重装这一个（[`super::fetch`]）：缺的新依赖一起装，设置、状态、开关都留着；新版多要的能力照旧
//!   要人批了才开（扩展的批准记在 `system/extensions.json`，核心照它起）。
//! - 盘点、换哪一层照 [`Scope`]（施工 U23 下上）：系统那一层的只看系统那一份，自己那一层的只看他自己装的（记在他那层的本地库）。
//! - 照装的时候记的软件源找（施工 U23 下下，本地库的 `feed`，没记的是官方的）：那个源从配置里删了的、这一层用不了的进 `gone`，
//!   不换到别的源上同名的包（换源就是换信任的对象）；升也照那个源装。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_source::index::Index;

use super::super::scope::Scope;
use super::feeds::OFFICIAL;
use super::fetch::{self, SourceParams};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::reverse::Peer as Reverse;
use crate::settings::SourceSettings;

/// 装着的一个：编号、清单写的版本、来源仓库（从软件源装的才有）、从哪个软件源装的（官方的没有，施工 U23 下下）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Installed {
    id: String,
    version: Option<String>,
    origin: Option<String>,
    feed: Option<String>,
}

/// 这一层用得上的几个源的索引：名字到索引（施工 U23 下下）。
type Indexes = BTreeMap<String, Index>;

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

/// `package.upgrades`：`scope` 那一层的，回 `{"upgrades", "moved", "gone", "blocked", "held", "generated", "stale"?}`。
pub(crate) async fn upgrades(core: &Arc<Core>, scope: &Scope) -> Result<Value, Refusal> {
    let synced = super::sync::sync(core, false).await?;
    let indexes = indexes(core, scope, &synced.index).await;
    let (survey, _) = survey_core(core, scope, &indexes).await?;
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
    (peer, scope): (Peer, &Scope),
    caller: Option<&Reverse>,
    params: UpgradeParams,
) -> Result<Value, Refusal> {
    let synced = super::sync::sync(core, false).await?;
    let indexes = indexes(core, scope, &synced.index).await;
    let (survey, feeds) = survey_core(core, scope, &indexes).await?;
    let feed_of = |id: &str| feeds.get(id).cloned().flatten();
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
            .filter_map(|(id, ..)| {
                let feed = feed_of(id).unwrap_or_else(|| OFFICIAL.to_string());
                indexes.get(&feed)?.entry(id)
            })
            .filter_map(fetch::pick)
            .map(|file| file.size)
            .sum();
        return Ok(json!({"upgrades": rows, "download": download}));
    }
    for (id, ..) in &wanted {
        let params = match feed_of(id) {
            Some(feed) => SourceParams::from(id, &feed),
            None => SourceParams::of(id),
        };
        fetch::install(core, (peer, scope), caller, params).await?;
    }
    Ok(json!({"upgraded": rows}))
}

/// `scope` 那一层用得上的几个源的索引（施工 U23 下下）：官方的是拿好的 `official`，别的照配置拿，拿不成的跳过（它装的包进
/// `gone`）。
async fn indexes(core: &Arc<Core>, scope: &Scope, official: &Index) -> Indexes {
    let account = match scope {
        Scope::System => None,
        Scope::Own(account) => Some(account),
    };
    let mut found = Indexes::new();
    found.insert(OFFICIAL.to_string(), official.clone());
    for feed in super::feeds::feeds(core, account).iter().skip(1) {
        if let Ok(synced) = super::sync::sync_feed(core, feed, false).await {
            found.insert(feed.name.clone(), synced.index);
        }
    }
    found
}

/// 照 `scope` 那一层装着的包、那一层的本地库、配置盘点：自己那一层的只算他自己装的。另交回每个包是从哪个源装的（官方的没有）。
async fn survey_core(
    core: &Arc<Core>,
    scope: &Scope,
    indexes: &Indexes,
) -> Result<(Survey, BTreeMap<String, Option<String>>), Refusal> {
    let mine = |found: &&miyu_store::packages::Found| match scope {
        Scope::System => true,
        Scope::Own(_) => found.layer == miyu_store::packages::Layer::User,
    };
    let found: Vec<(String, Option<String>)> = scope
        .installed(core)
        .iter()
        .filter(mine)
        .map(|found| {
            let version = found.read.as_ref().ok().and_then(|m| m.version.clone());
            (found.id.clone(), version)
        })
        .collect();
    let root = scope.local_root(core);
    let installed = tokio::task::spawn_blocking(move || {
        found
            .into_iter()
            .map(|(id, version)| {
                let desc = miyu_store::packages::local::read(&root, &id).map(|(desc, _)| desc);
                let (origin, feed) = desc.map_or((None, None), |desc| (desc.origin, desc.feed));
                Installed {
                    id,
                    version,
                    origin,
                    feed,
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let hold = SourceSettings::from(&core.config().resolved().values())
        .hold
        .unwrap_or_default();
    let feeds = installed
        .iter()
        .map(|one| (one.id.clone(), one.feed.clone()))
        .collect();
    Ok((survey(&installed, indexes, &hold), feeds))
}

/// 盘点：只看记了来源仓库的，照它装的时候那个源的索引比（施工 U23 下下）；那个源这一层用不了的进 `gone`。
fn survey(installed: &[Installed], indexes: &Indexes, hold: &[String]) -> Survey {
    let mut survey = Survey::default();
    for one in installed {
        let Some(origin) = &one.origin else {
            continue;
        };
        let Some(index) = indexes.get(one.feed.as_deref().unwrap_or(OFFICIAL)) else {
            survey.gone.push(one.id.clone());
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

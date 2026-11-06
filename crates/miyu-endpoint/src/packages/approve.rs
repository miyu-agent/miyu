//! 装的时候就批能力（施工 PK-3 上，设计 `61-装包说清来源和能力.md` 第四节）：`package.install`、`package.upgrade` 的 `approve`
//! 是这一次装上的每个包各批哪几样（编号到能力名）。装成以后照
//!
//! ```text
//! 接着用 = 旧的 approved[编号]，只在「来路没变」时才算，不然是空的
//! 新的 approved[编号] = (接着用 ∪ approve[编号]) ∩ 这一份清单声明的
//! ```
//!
//! 重算这个包在开关文件 `extensions.json` 里的 `approved`（形状不变），在拉起扩展之前写好：`start = "always"` 的一装上就批过了，
//! 当场拉起。来路没变是装之前本地库 `desc` 记的和这一次一样都是从软件源装的、来源仓库和软件源都一样；本机文件装的、换了源的、
//! 换了来源仓库的、本地库没记的都算变了，前面批过的不接着用。看一眼的 `held`（这一刻批过、装完照旧留着的）照同一套算。

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use miyu_config::package::{Capability, Manifest};
use miyu_store::packages::local::{self, Desc};

use super::TARGET;
use crate::Core;
use crate::refusal::Refusal;

/// `approve` 参数：包的编号到这一次批的能力名。
pub(crate) type Approve = BTreeMap<String, Vec<String>>;

/// 写法对不对：编号照包编号的写法、能力名是十九个之一，不对的 `bad_params`。是能力、但清单没声明的这里不管（装的时候不理：
/// 看一眼和真装之间清单可能变了），编号不是这一次装的也不管。
pub(crate) fn valid(approve: Option<&Approve>) -> Result<(), Refusal> {
    let fine = approve.into_iter().flatten().all(|(id, names)| {
        miyu_store::personas::valid(id)
            && names.iter().all(|name| Capability::parse(name).is_some())
    });
    fine.then_some(()).ok_or(Refusal::BAD_PARAMS)
}

/// 从软件源装的来路：来源仓库、软件源的名字（官方的没有，同本地库 `desc` 的 `feed`）、官方还是社区收录的（施工 PK-6，同看一眼的
/// `source`）。从本机文件夹、包文件装的没有来路。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Came {
    pub(crate) origin: String,
    pub(crate) feed: Option<String>,
    pub(crate) kind: miyu_store::packages::local::Kind,
}

/// 一次装的来路和批了什么：从软件源装的带 `came`，`approve` 是请求里的那一张表（没写的没有）。
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Arrival<'a> {
    pub(crate) approve: Option<&'a Approve>,
    pub(crate) came: Option<&'a Came>,
}

/// 来路没变：装之前本地库记的 `old` 和这一次 `came` 一样都是从软件源装的，来源仓库、软件源都一样。
pub(crate) fn same_way(old: Option<&Desc>, came: Option<&Came>) -> bool {
    match (old, came) {
        (Some(old), Some(came)) => {
            old.origin.as_deref() == Some(came.origin.as_str()) && old.feed == came.feed
        }
        _ => false,
    }
}

/// 重算的批准：(接着用的 `kept` ∪ 这一次批的 `approve`) ∩ 声明的 `declared`，照名字排、不重。
pub(crate) fn settled(kept: &[String], approve: &[String], declared: &[Capability]) -> Vec<String> {
    let mut names: Vec<String> = declared
        .iter()
        .map(|capability| capability.as_str())
        .filter(|name| kept.iter().chain(approve).any(|one| one == name))
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// 包 `id` 这一刻批过、照来路 `came` 装完还接着用的能力名（看一眼的 `held`）：来路变了的没有。读本地库 `local` 的 `desc`。
pub(crate) fn kept(
    approved: &BTreeMap<String, Vec<String>>,
    local: &Path,
    id: &str,
    came: Option<&Came>,
) -> Vec<String> {
    match same_way(local::desc(local, id).as_ref(), came) {
        true => approved.get(id).cloned().unwrap_or_default(),
        false => Vec::new(),
    }
}

/// 看一眼的能力那一格（`[{"id", "name", "summary"}]`）里，`held` 里有的那几项标上 `held: true`。
pub(crate) fn mark_held(listed: &mut Value, held: &[String]) {
    for one in listed.as_array_mut().into_iter().flatten() {
        if one["id"]
            .as_str()
            .is_some_and(|id| held.iter().any(|name| name == id))
        {
            one["held"] = Value::Bool(true);
        }
    }
}

/// 装成了包 `id`（读成了的清单 `manifest`，读坏了的当没声明）以后重算它的批准、写回开关文件（施工 PK-3 上）：拿着开关那把锁
/// 读、改、写，`kept` 是来路没变、旧的接着用。没变的不写。写不进的记一行 `WARN extension approval not recorded`，装照算成：没批上
/// 的照旧停在「要批准」，打开时再批。
pub(super) async fn record(
    core: &Core,
    (id, manifest): (&str, Option<&Manifest>),
    kept: bool,
    approve: &[String],
) {
    let _one_at_a_time = core.extensions.one_at_a_time().await;
    let (mut switches, version) = crate::extensions::read_switches(core);
    let old = switches.approved.get(id).cloned().unwrap_or_default();
    let declared = manifest.map_or(&[][..], crate::extensions::declared);
    let kept: &[String] = if kept { &old } else { &[] };
    let now = settled(kept, approve, declared);
    if now == old {
        return;
    }
    match now.is_empty() {
        true => switches.approved.remove(id),
        false => switches.approved.insert(id.to_string(), now),
    };
    let path = core.root.extensions_file();
    if let Err(error) = miyu_store::extensions::write(&path, &switches, version.as_deref()) {
        tracing::warn!(target: TARGET, package = id, error = %error, "extension approval not recorded");
    }
}

/// 照看一眼的那一份 `looked` 拼 `approve`（她的工具，施工 PK-3 上：问人的卡片上那一份就是批的那一份）：要装的那一个（顶上的
/// `package`、`capabilities`）、一起装的（`with`）、升级的（`upgrades`）各自列出来的能力。推荐的不算：这一次不装它们。
pub(crate) fn of_looked(looked: &Value) -> Approve {
    let mut approve = Approve::new();
    let items = std::iter::once(looked)
        .chain(looked["with"].as_array().into_iter().flatten())
        .chain(looked["upgrades"].as_array().into_iter().flatten());
    for item in items {
        let (Some(id), Some(listed)) = (item["package"].as_str(), item["capabilities"].as_array())
        else {
            continue;
        };
        let names: Vec<String> = listed
            .iter()
            .filter_map(|one| one["id"].as_str().map(str::to_string))
            .collect();
        if !names.is_empty() {
            approve.insert(id.to_string(), names);
        }
    }
    approve
}

#[cfg(test)]
mod tests;

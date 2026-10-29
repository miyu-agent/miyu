//! 扩展能力的审批（施工 9-4 下上，`docs/blueprint/extensions.md`「能力」，`05-内核接口.md` 第三节）：清单的
//! `[process] capabilities` 声明要哪些，人在 `extension.enable` 那一下带上 `approve` 批。批了就是记进开关文件
//! `extensions.json` 的 `approved`（施工 SU-2 中：组表拿掉了），批的时候声明的全部。还没批的是「声明的减记着的」：升级
//! 以后多了的只问多的。出厂的包（资源目录里随 Miyu 装的）和编译进核心的模块一样本来就被信任，算批过的、不记。

use serde_json::{Value, json};

use miyu_config::package::{Capability, Manifest};
use miyu_config::words::Words;
use miyu_store::extensions::Switches;
use miyu_store::packages::Layer;

use crate::Core;
use crate::refusal::Refusal;

/// 包声明的能力，照能力表的先后。
fn declared(manifest: &Manifest) -> &[Capability] {
    manifest
        .process
        .as_ref()
        .map_or(&[], |process| process.capabilities.as_slice())
}

/// 包 `id` 是不是出厂的。
fn shipped(core: &Core, id: &str) -> bool {
    core.system_packages()
        .iter()
        .any(|found| found.id == id && found.layer == Layer::Shipped)
}

/// 包 `id` 手里有的能力，照能力表的先后：出厂的是声明的全部，别的是声明的里面开关文件记着批过的。
fn holds(core: &Core, id: &str, manifest: &Manifest, switches: &Switches) -> Vec<Capability> {
    let shipped = shipped(core, id);
    let approved = switches.approved.get(id).map_or(&[][..], Vec::as_slice);
    declared(manifest)
        .iter()
        .copied()
        .filter(|capability| shipped || approved.iter().any(|one| one == capability.as_str()))
        .collect()
}

/// 包 `id` 还没批的能力，照能力表的先后：出厂的没有。
pub(super) fn unapproved(core: &Core, id: &str, manifest: &Manifest) -> Vec<Capability> {
    let held = holds(core, id, manifest, &super::read_switches(core).0);
    declared(manifest)
        .iter()
        .copied()
        .filter(|capability| !held.contains(capability))
        .collect()
}

/// 包 `package` 的连接调方法时手里有的能力（`connection/rights.rs` 照它挡）：批过的（出厂的是声明的全部）。
pub(crate) fn held(core: &Core, package: &str) -> Vec<Capability> {
    let packages = core.system_packages();
    let Some(manifest) = packages
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok())
    else {
        return Vec::new();
    };
    holds(core, package, manifest, &super::read_switches(core).0)
}

/// 开包 `id` 之前批，记进正要写回的开关 `switches`（调的一方拿着开关那把锁、读好、批完一起写回）：`approve` 里有不是它声明的名字
/// 的 `bad_params`；还有没批、`approve` 没盖住的 `needs_approval`（`data.capabilities` 是没盖住的那几个）；盖住了的记下它这时
/// 声明的全部。都批过了的不动。
pub(super) fn approve(
    core: &Core,
    (id, manifest): (&str, &Manifest),
    approve: Option<&[String]>,
    switches: &mut Switches,
) -> Result<(), Refusal> {
    let declared = declared(manifest);
    let named = approve.unwrap_or_default();
    let known = |name: &String| Capability::parse(name).is_some_and(|it| declared.contains(&it));
    if !named.iter().all(known) {
        return Err(Refusal::BAD_PARAMS);
    }
    let held = holds(core, id, manifest, switches);
    let missing: Vec<Capability> = declared
        .iter()
        .copied()
        .filter(|capability| !held.contains(capability))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let left: Vec<Capability> = missing
        .into_iter()
        .filter(|capability| !named.iter().any(|name| name == capability.as_str()))
        .collect();
    if !left.is_empty() {
        return Err(needs_approval(&left));
    }
    let mut names: Vec<String> = declared.iter().map(|it| it.as_str().to_string()).collect();
    names.sort();
    switches.approved.insert(id.to_string(), names);
    Ok(())
}

/// 还有没批的：拒绝 `needs_approval`，`data.capabilities` 是这几个。
pub(super) fn needs_approval(left: &[Capability]) -> Refusal {
    let names: Vec<&str> = left.iter().map(|it| it.as_str()).collect();
    Refusal::needs_approval(json!(names))
}

/// 给人看的那两格：声明的每一个 `{"id", "name", "summary"}`（名字、一句说明照 `language` 挑，没有字的名字是编号、说明是
/// `null`），还有没批的名字。没声明能力的包都没有。
pub(super) fn shown(
    core: &Core,
    id: &str,
    manifest: &Manifest,
    language: &str,
) -> Option<(Value, Option<Value>)> {
    let listed = described(core, manifest, language)?;
    let left = unapproved(core, id, manifest);
    let left =
        (!left.is_empty()).then(|| json!(left.iter().map(|it| it.as_str()).collect::<Vec<_>>()));
    Some((listed, left))
}

/// 清单声明的每一个能力 `{"id", "name", "summary"}`（名字、一句说明照 `language` 挑，没有字的名字是编号、说明是 `null`）；
/// 没声明能力的没有。装之前看一眼也照它说（施工 F-8 下补）。
pub(crate) fn described(core: &Core, manifest: &Manifest, language: &str) -> Option<Value> {
    let declared = declared(manifest);
    if declared.is_empty() {
        return None;
    }
    Some(named(core, declared.iter().map(|it| it.as_str()), language))
}

/// 几个能力的编号写成 `[{"id", "name", "summary"}]`，同 [`described`]：软件源索引里写的能力（施工 IN-2，`package.info` 查
/// 软件源的那一份）也照它说。
pub(crate) fn named<'a>(core: &Core, ids: impl Iterator<Item = &'a str>, language: &str) -> Value {
    let words = crate::config::methods::words(core, language).ok();
    let said = |key: String| {
        words
            .as_ref()
            .and_then(|words| Words::sentence(words, &key, &[]))
    };
    let listed: Vec<Value> = ids
        .map(|id| {
            let key = format!("extensions/capabilities/{id}");
            json!({
                "id": id,
                "name": said(key.clone()).unwrap_or_else(|| id.to_string()),
                "summary": said(format!("{key}/summary")),
            })
        })
        .collect();
    json!(listed)
}

/// 卸掉了包 `id`（施工 F-8 五补，设计 31 第三节第 3 条「重装回来从头设」）：开关的文件里它开没开、批过什么一起删掉；都没有的不写
/// 文件。写不成的记一行 `WARN`，卸照样算成了。
pub(crate) async fn forget(core: &Core, id: &str) {
    let _one_at_a_time = core.extensions.one_at_a_time().await;
    let (mut switches, version) = super::read_switches(core);
    let on = switches.on.remove(id).is_some();
    let approved = switches.approved.remove(id).is_some();
    if (on || approved)
        && let Err(refusal) = super::write_switches(core, id, &switches, version.as_deref())
    {
        tracing::warn!(target: super::TARGET, package = id, reason = refusal.reason, "extension switch not forgotten");
    }
}

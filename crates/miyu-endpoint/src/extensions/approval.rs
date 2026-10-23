//! 扩展能力的审批（施工 9-4 下上，`docs/blueprint/extensions.md`「能力」，`05-内核接口.md` 第三节）：清单的
//! `[process] capabilities` 声明要哪些，管理员在 `extension.enable` 那一下带上 `approve` 批。批了就是把包的账号加进那几个能力的
//! 组（施工 Y-5 下上，设计 06 U16「能力即组」：组表 `system/groups.json` 的 `ext.<能力>`，[`group_of`]），批的时候声明的全部。
//! 还没批的是「声明的减组里有的」：升级以后多了的只问多的。出厂的包（资源目录里随 Miyu 装的）和编译进核心的模块一样本来就被信任，
//! 算批过的、不进组表。以前记在开关文件里的 `approved` 不再认：换核心以后要重新批一次（开关照旧）。

use serde_json::{Value, json};

use miyu_config::package::{Capability, Manifest};
use miyu_config::words::Words;
use miyu_kernel::id::AccountId;
use miyu_store::groups::Groups;
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
    core.packages()
        .iter()
        .any(|found| found.id == id && found.layer == Layer::Shipped)
}

/// 能力 `capability` 的组名：`ext.<能力>`（施工 Y-5 下上）。
fn group_of(capability: Capability) -> String {
    format!("ext.{}", capability.as_str())
}

/// 包 `id` 手里有的能力，照能力表的先后（施工 Y-5 下上）：出厂的是声明的全部，别的是声明的里面组表批过的。
fn holds(core: &Core, id: &str, manifest: &Manifest, groups: &Groups) -> Vec<Capability> {
    let shipped = shipped(core, id);
    declared(manifest)
        .iter()
        .copied()
        .filter(|capability| shipped || groups.has(&group_of(*capability), id))
        .collect()
}

/// 包 `id` 还没批的能力，照能力表的先后：出厂的没有。
pub(super) fn unapproved(core: &Core, id: &str, manifest: &Manifest) -> Vec<Capability> {
    let held = holds(core, id, manifest, &crate::login::read_groups(core).groups);
    declared(manifest)
        .iter()
        .copied()
        .filter(|capability| !held.contains(capability))
        .collect()
}

/// 包 `package` 的连接调方法时手里有的能力（施工 Y-5 下上，`connection/rights.rs` 照它挡）：组表批过的（出厂的是声明的全部）。
/// 它替人干活的（`serving`，设计 06 U17），那个人自己也得有这项权限，取交集：这一版人这一侧照「是不是管理员」（机主恒是），
/// 成员也能用扩展的时候照名单换，判法不变。
pub(crate) fn held(core: &Core, package: &str, serving: Option<&AccountId>) -> Vec<Capability> {
    if serving.is_some_and(|person| !crate::login::admins::is_admin(core, person)) {
        return Vec::new();
    }
    let packages = core.packages();
    let Some(manifest) = packages
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok())
    else {
        return Vec::new();
    };
    holds(
        core,
        package,
        manifest,
        &crate::login::read_groups(core).groups,
    )
}

/// 开包 `id` 之前批：`approve` 里有不是它声明的名字的 `bad_params`；还有没批、`approve` 没盖住的 `needs_approval`
/// （`data.capabilities` 是没盖住的那几个）；盖住了的把包的账号加进它这时声明的每一个能力的组（施工 Y-5 下上）。都批过了的不动。
pub(super) async fn approve(
    core: &Core,
    id: &str,
    manifest: &Manifest,
    approve: Option<&[String]>,
) -> Result<(), Refusal> {
    let declared = declared(manifest);
    let named = approve.unwrap_or_default();
    let known = |name: &String| Capability::parse(name).is_some_and(|it| declared.contains(&it));
    if !named.iter().all(known) {
        return Err(Refusal::BAD_PARAMS);
    }
    let missing = unapproved(core, id, manifest);
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
    let groups: Vec<String> = declared.iter().map(|it| group_of(*it)).collect();
    crate::login::change_groups(core, |table| {
        for group in &groups {
            table.add(group, id);
        }
        Ok(())
    })
    .await
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
    let words = crate::config::methods::words(core, language).ok();
    let said = |key: String| {
        words
            .as_ref()
            .and_then(|words| Words::sentence(words, &key, &[]))
    };
    let listed: Vec<Value> = declared
        .iter()
        .map(|capability| {
            let key = format!("extensions/capabilities/{}", capability.as_str());
            json!({
                "id": capability.as_str(),
                "name": said(key.clone()).unwrap_or_else(|| capability.as_str().to_string()),
                "summary": said(format!("{key}/summary")),
            })
        })
        .collect();
    Some(json!(listed))
}

/// 卸掉了包 `id`（施工 F-8 五补，设计 31 第三节第 3 条「重装回来从头设」）：开关的文件里它开没开删掉，它在的 `ext.*` 组一起
/// 移出（施工 Y-5 下上）；都没有的不写文件。写不成的记一行 `WARN`，卸照样算成了。
pub(crate) async fn forget(core: &Core, id: &str) {
    let _one_at_a_time = core.extensions.one_at_a_time().await;
    let (mut switches, version) = super::read_switches(core);
    if switches.on.remove(id).is_some()
        && let Err(refusal) = super::write_switches(core, id, &switches, version.as_deref())
    {
        tracing::warn!(target: super::TARGET, package = id, reason = refusal.reason, "extension switch not forgotten");
    }
    let held: Vec<String> = crate::login::read_groups(core)
        .groups
        .groups
        .iter()
        .filter(|(name, members)| name.starts_with("ext.") && members.iter().any(|one| one == id))
        .map(|(name, _)| name.clone())
        .collect();
    if held.is_empty() {
        return;
    }
    let removed = crate::login::change_groups(core, |table| {
        for group in &held {
            table.remove(group, id);
        }
        Ok(())
    })
    .await;
    if let Err(refusal) = removed {
        tracing::warn!(target: super::TARGET, package = id, reason = refusal.reason, "extension approvals not forgotten");
    }
}

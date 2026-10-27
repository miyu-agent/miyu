//! 装、卸整个组（施工 BS-1 下中，设计 `44-基础系统拆包.md` 第三节第 2 条，照 pacman 写组名）：`package.install {"package": <组>}`
//! 把组里卸掉了的出厂包一个个装回来，`package.remove {"package": <组>}` 把组里装着的一个个卸掉；都带 `preview` 的只看一眼。
//!
//! - 认组：组表（`resources/packages/groups.toml`）里有、又不是哪个出厂包的编号的（组和包撞名的先认包，出厂资源的测试守着不撞）。
//! - 组里有哪些：出厂清单写了这个组的包（`[package] groups`），照编号排。
//! - 卸：组外装着的包依赖组里的，整个拒（`package_depended`，同卸一个），什么都不卸；组里互相依赖的照依赖的先后卸（先卸依赖别人的）。
//! - 回应：装回的 `{"group", "packages": [<列表里那一项>…]}`，卸的 `{"group", "removed": [<编号>…]}`；看一眼的 `{"group",
//!   "packages": [<编号>…]}`，是这一次会装回、会卸掉的那几个。一个都没有的照样回，列表是空的。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_config::package::groups::Group;
use miyu_store::packages::Packages;

use super::{TARGET, depends, packages};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 出厂的组表，照写的先后；读不成的记一行、当没有。
pub(super) fn table(core: &Core) -> Vec<Group> {
    let path = core.resources.path().join("packages").join("groups.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => miyu_config::package::groups::read(&text).unwrap_or_else(|error| {
            tracing::warn!(target: TARGET, error = %error, "package groups not read");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}

/// `id` 是不是一个组：组表里有、又不是哪个出厂包的编号。
pub(crate) fn is_group(core: &Core, id: &str) -> bool {
    let shipped = Packages::shipped(&core.resources).read();
    !shipped.iter().any(|one| one.id == id) && table(core).iter().any(|group| group.id == id)
}

/// 参数里的 `package` 是组名（`package.install`、`package.remove` 照它分到 [`call`]）。
pub(crate) fn named(core: &Core, params: &Value) -> bool {
    params
        .get("package")
        .and_then(Value::as_str)
        .is_some_and(|id| is_group(core, id))
}

/// 写组名的参数：只认这两格。
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GroupParams {
    package: String,
    #[serde(default)]
    preview: bool,
}

impl GroupParams {
    /// 组 `package`，`preview` 的只看一眼（她的工具经端口装回组用，施工 BS-1 中下修）。
    pub(crate) fn of(package: &str, preview: bool) -> GroupParams {
        GroupParams {
            package: package.to_string(),
            preview,
        }
    }
}

/// 写组名的 `package.install`、`package.remove`。
pub(crate) async fn call(
    core: &Arc<Core>,
    peer: Peer,
    method: &str,
    params: GroupParams,
) -> Result<Value, Refusal> {
    match method {
        "package.remove" => remove(core, &params.package, params.preview).await,
        _ => restore(core, peer, &params.package, params.preview).await,
    }
}

/// 组 `group` 里的包：出厂清单写了它的，照编号。
fn members(core: &Core, group: &str) -> Vec<String> {
    Packages::shipped(&core.resources)
        .read()
        .iter()
        .filter(|one| {
            one.read
                .as_ref()
                .is_ok_and(|manifest| manifest.groups.iter().any(|name| name == group))
        })
        .map(|one| one.id.clone())
        .collect()
}

/// 把组里卸掉了的出厂包装回来；`preview` 的只交回会装回的那几个。
async fn restore(
    core: &Arc<Core>,
    peer: Peer,
    group: &str,
    preview: bool,
) -> Result<Value, Refusal> {
    let removed = packages(core).removed();
    let back: Vec<String> = members(core, group)
        .into_iter()
        .filter(|id| removed.contains(id))
        .collect();
    if preview {
        return Ok(json!({"group": group, "packages": back}));
    }
    let mut entries = Vec::new();
    for id in &back {
        entries.push(super::manage::bring_back(core, peer, id).await?);
    }
    Ok(json!({"group": group, "packages": entries}))
}

/// 把组里装着的包卸掉；`preview` 的只交回会卸掉的那几个。组外装着的包依赖组里的整个拒。
async fn remove(core: &Arc<Core>, group: &str, preview: bool) -> Result<Value, Refusal> {
    let installed = core.system_packages();
    let mut left: Vec<String> = members(core, group)
        .into_iter()
        .filter(|id| installed.iter().any(|one| &one.id == id))
        .collect();
    let mut outside: Vec<String> = left
        .iter()
        .flat_map(|id| depends::dependents(&installed, id))
        .filter(|by| !left.contains(by))
        .collect();
    outside.sort();
    outside.dedup();
    if !outside.is_empty() {
        return Err(Refusal::package_depended(&outside));
    }
    if preview {
        return Ok(json!({"group": group, "packages": left}));
    }
    let mut removed = Vec::new();
    while !left.is_empty() {
        let now = core.system_packages();
        // 先卸组里没有别的包依赖着的；一个都挑不出的（不该有：依赖成了环）照剩下的先后卸，单个卸时照常拒。
        let at = left
            .iter()
            .position(|id| depends::dependents(&now, id).is_empty())
            .unwrap_or(0);
        let id = left.remove(at);
        super::manage::remove_package(core, id.clone()).await?;
        removed.push(id);
    }
    Ok(json!({"group": group, "removed": removed}))
}

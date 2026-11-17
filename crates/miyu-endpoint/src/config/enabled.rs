//! 包的「启用」在协议上（施工 UX-1，项目主人 2026-11-17，`miyu_config::package::enabled`）：设置页上每个有开关的包那一组底下
//! 一项 `packages.<编号>.enabled`，值就是 `package.list` 的 `enabled`。它不写在配置文件里，所以不在配置清单里：
//!
//! - `config.schema` 照这一刻有开关的包现拼，排在那个包自己的配置项前面（[`place`]）；
//! - `config.get` 交它这时开没开，来源写 `{"layer": "package"}`；
//! - `config.set` 改它就是 `package.enable`、`package.disable`（[`set`]）：同一条里别的项先照常写进配置，再一个个开关包。
//!   开不起来的（要批的能力还没批、程序不在……）照 `package.enable` 的拒绝原样回，前面已经写进配置的照旧留着。
//!
//! 没有开关的包（必需的、界面、小程序、只带技能脚本的 `bundle`）没有这一项，写了照不认识的键。

use std::sync::Arc;

use serde_json::{Map, Value as Json, json};

use miyu_config::package::enabled::{id_of, item_for};
use miyu_config::{Item, Kind, Value, edit};
use miyu_kernel::id::CommandId;
use miyu_store::packages::Layer;

use super::set::{ChangeParam, SetParams};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 来源：不是哪一层配置，是包自己的开关。
fn origin() -> Json {
    json!({"layer": "package"})
}

/// 这一刻有开关的包和开没开，照 `package.list` 的先后：装着的，再是卸掉了的出厂包（开关同 `package.list` 的 `enabled`）。
pub(crate) fn switches(core: &Core) -> Vec<(String, bool)> {
    let system = core.system_packages();
    let removed = crate::packages::packages(core).read_removed();
    let gone = removed
        .iter()
        .filter(|found| !system.iter().any(|one| one.id == found.id));
    system
        .iter()
        .map(|found| (found, false))
        .chain(gone.map(|found| (found, true)))
        .filter_map(|(found, removed)| {
            let manifest = found.read.as_ref().ok()?;
            let shipped = found.layer == Layer::Shipped;
            crate::packages::status::enabled(core, &found.id, manifest, shipped, removed)
                .map(|on| (found.id.clone(), on))
        })
        .collect()
}

/// 请求里的 `keys` 分成两份：有开关的包的「启用」，和别的（照常认，认不出的照旧 `unknown_config_key`）。没写 `keys` 的是全部
/// 「启用」、别的照旧不写。
pub(crate) fn split(
    switches: &[(String, bool)],
    keys: Option<&[String]>,
) -> (Vec<(String, bool)>, Option<Vec<String>>) {
    let Some(keys) = keys else {
        return (switches.to_vec(), None);
    };
    let mut asked = Vec::new();
    let mut rest = Vec::new();
    for key in keys {
        let found = id_of(key).and_then(|id| switches.iter().find(|(one, _)| *one == id));
        match found {
            Some(found) if !asked.contains(found) => asked.push(found.clone()),
            Some(_) => {}
            None => rest.push(key.clone()),
        }
    }
    (asked, Some(rest))
}

/// 把「启用」放进 `config.schema` 的项里：排在那个包自己的第一项前面，包没有别的项的排在最后。
pub(crate) fn place(items: &mut Vec<&Item>, switches: &[(String, bool)]) {
    for (id, _) in switches {
        let item = item_for(id);
        let at = items
            .iter()
            .position(|one| one.ui.package == Some(id.as_str()))
            .unwrap_or(items.len());
        items.insert(at, item);
    }
}

/// `config.get` 里的一项：这时开没开；问了每一层的（`all`）没有哪一层写了它。
pub(crate) fn got(listed: &mut Map<String, Json>, switches: &[(String, bool)], all: bool) {
    for (id, on) in switches {
        let mut entry = json!({"origin": origin(), "value": on});
        if all {
            entry["layers"] = json!([]);
        }
        listed.insert(item_for(id).key.to_string(), entry);
    }
}

/// 人改配置的 `config.set`：改「启用」的几项开关包，别的照常写进配置（[`super::set::set`]）。交回合起来的回应，「启用」那几项
/// 同 `keys` 的样子：什么时候生效、这时开没开（`effective`、`value` 一样）、来源。
pub(crate) async fn set(
    core: &Arc<Core>,
    peer: Peer,
    cause: &CommandId,
    mut params: SetParams,
) -> Result<Json, Refusal> {
    let wanted = taken(&switches(core), &mut params)?;
    if wanted.is_empty() {
        return super::set::set(core, peer, cause, params);
    }
    let mut reply = match params.changes.as_ref().is_some_and(|left| !left.is_empty()) {
        true => super::set::set(core, peer, cause, params)?,
        false => json!({"keys": {}, "version": core.config().file().version}),
    };
    for (id, on) in wanted {
        let entry = crate::packages::switch::turn(core, peer, &id, on).await?;
        let now = entry["enabled"].as_bool().unwrap_or(on);
        reply["keys"][item_for(&id).key] =
            json!({"applies": "now", "effective": now, "origin": origin(), "value": now});
    }
    Ok(reply)
}

/// 从 `params` 里拿出改「启用」的几项：包的编号、要开还是关。写法只收开关（`value` 写 `true`、`false`，或者 `input`
/// 照开关读），删掉的、同一个键写两次的不收（`bad_params`）；`expect` 不看（不在哪一层里）；整份换的不碰。
fn taken(
    switches: &[(String, bool)],
    params: &mut SetParams,
) -> Result<Vec<(String, bool)>, Refusal> {
    let Some(changes) = params.changes.take() else {
        return Ok(Vec::new());
    };
    let (mine, rest): (Vec<ChangeParam>, Vec<ChangeParam>) =
        changes.into_iter().partition(|change| {
            id_of(&change.key).is_some_and(|id| switches.iter().any(|(one, _)| *one == id))
        });
    params.changes = Some(rest);
    let mut wanted: Vec<(String, bool)> = Vec::new();
    for change in mine {
        let id = id_of(&change.key).ok_or(Refusal::BAD_PARAMS)?;
        let on = match (&change.value, &change.input, change.plain()) {
            (Some(Json::Bool(on)), None, true) => *on,
            (None, Some(input), true) => match edit::input(Kind::Bool, input) {
                Some(Value::Bool(on)) => on,
                _ => return Err(Refusal::BAD_PARAMS),
            },
            _ => return Err(Refusal::BAD_PARAMS),
        };
        if wanted.iter().any(|(seen, _)| *seen == id) {
            return Err(Refusal::BAD_PARAMS);
        }
        wanted.push((id, on));
    }
    Ok(wanted)
}

#[cfg(test)]
mod tests;

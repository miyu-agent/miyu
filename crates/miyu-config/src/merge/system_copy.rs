//! 两层都配了的供应商，另合一份系统配置的原样（施工 PL-1，2026-10-24 项目主人定「自用、共用两份都能用」）：同一个编号
//! 系统配置、个人设置都写了，`providers.<编号>.*` 照旧是个人设置逐格盖过系统配置的那一份（自用），另外多出
//! `providers."<编号>@system".*`，只照系统配置合（共用的原样）。它引用的密钥只从系统那一份取
//! （[`Reference::SystemSecret`]）：自己存了同名的也不顶掉它。只在最终值里有，文件里写不出（`@` 不在编号的写法里）。
//! 引用写成 `<编号>@system/<模型>`；冷却、用量、模型列表照编号各记各的。

use std::collections::BTreeSet;

use super::{Layers, Resolved, merge};
use crate::item::Item;
use crate::key::{self, ID};
use crate::secret::Reference;
use crate::value::Value;

/// 把系统那一份的原样加进 `resolved`：`layers` 里系统配置、个人设置都写了的供应商编号，照只有系统配置的那几层再合一次，
/// 键里的编号换成 [`key::system_copy`]。
pub(super) fn add(
    items: &[Item],
    layers: &Layers,
    env: &dyn Fn(&str) -> Option<String>,
    resolved: &mut Resolved,
) {
    let (Some(system), Some(personal)) = (layers.system, layers.personal) else {
        return;
    };
    let pattern = format!("providers.{ID}");
    let written =
        |entries: &std::collections::BTreeMap<String, crate::parse::Entry>| -> BTreeSet<String> {
            key::names(entries.keys().map(String::as_str), &pattern, &[])
                .into_iter()
                .collect()
        };
    let both: BTreeSet<String> = written(&system.entries)
        .intersection(&written(&personal.entries))
        .cloned()
        .collect();
    if both.is_empty() {
        return;
    }
    let only_system = Layers {
        system: Some(system),
        personal: None,
        project: None,
    };
    let alone = merge(items, &only_system, env);
    for (real, (value, origin)) in alone.entries {
        let Some(mut segments) = key::split(&real) else {
            continue;
        };
        let copied = match segments.as_slice() {
            [first, id, _, ..] if first == "providers" && both.contains(id) => key::system_copy(id),
            _ => continue,
        };
        segments[1] = copied;
        resolved
            .entries
            .insert(key::join(&segments), (system_only(value), origin));
    }
}

/// 引用的密钥换成只从系统那一份取的。
fn system_only(value: Value) -> Value {
    match value {
        Value::Secret(Reference::Secret(name)) => Value::Secret(Reference::SystemSecret(name)),
        Value::List(values) => Value::List(values.into_iter().map(system_only).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests;

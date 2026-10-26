//! 两层都配了的供应商，另合一份系统配置的原样（施工 PL-1，2026-10-24 项目主人定「自用、共用两份都能用」）：同一个编号
//! 系统配置、个人设置都写了，`providers.<编号>.*` 照旧是个人设置逐格盖过系统配置的那一份（自用），另外多出
//! `providers."<编号>@system".*`，只照系统配置合（共用的原样）。它引用的密钥只从系统那一份取
//! （[`Reference::SystemSecret`]）：自己存了同名的也不顶掉它。只在最终值里有，文件里写不出（`@` 不在编号的写法里）。
//! 引用写成 `<编号>@system/<模型>`；冷却、用量、模型列表照编号各记各的。
//!
//! 只有系统配置写了的供应商（[`unfold`]，施工 PL-1 修）：没有两份，`<编号>@system` 指的就是它本身。人选过共用那份、后来删了
//! 自用那份，引用照旧算数：最终值里写成 `<编号>/<模型>`，[`Resolved::aliases`] 记下这一对，查引用在不在时照它认。

use std::collections::BTreeSet;

use super::{Layers, Resolved, merge};
use crate::item::{Item, Kind, Pointed, pointed};
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
    let both: BTreeSet<String> = written(system)
        .intersection(&written(personal))
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

/// 一层配置 `parsed` 里写了的供应商编号。
fn written(parsed: &crate::parse::Parsed) -> BTreeSet<String> {
    let pattern = format!("providers.{ID}");
    key::names(parsed.entries.keys().map(String::as_str), &pattern, &[])
        .into_iter()
        .collect()
}

/// 只有系统配置写了的供应商（个人设置没写）：`<编号>@system` 记成它本身的别名，引用、模型列表里写成 `<编号>@system/<模型>` 的
/// 在最终值里换成 `<编号>/<模型>`。
pub(super) fn unfold(items: &[Item], layers: &Layers, resolved: &mut Resolved) {
    let Some(system) = layers.system else {
        return;
    };
    let mine = layers.personal.map(written).unwrap_or_default();
    for id in written(system).difference(&mine) {
        resolved.aliases.insert(key::system_copy(id), id.clone());
    }
    if resolved.aliases.is_empty() {
        return;
    }
    let aliases = resolved.aliases.clone();
    for (item, key) in super::concrete(items, layers) {
        if !refers(item.kind) {
            continue;
        }
        if let Some((value, _)) = resolved.entries.get_mut(&key) {
            unfold_value(value, &aliases);
        }
    }
}

/// 这一类的值是引用、模型、模型的列表（同 [`crate::dangling`] 查的那几类）。
fn refers(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Reference
            | Kind::Model
            | Kind::ModelOr(_)
            | Kind::List(Kind::Reference | Kind::Model)
    )
}

/// 值里指着别名的引用换成指着它本身。
fn unfold_value(value: &mut Value, aliases: &std::collections::BTreeMap<String, String>) {
    match value {
        Value::Text(text) => {
            if let Some(Pointed::Provider(name)) = pointed(text)
                && let Some(id) = aliases.get(name)
            {
                let rest = text[name.len()..].to_string();
                *value = Value::Text(format!("{id}{rest}").into());
            }
        }
        Value::List(values) => values
            .iter_mut()
            .for_each(|value| unfold_value(value, aliases)),
        _ => {}
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

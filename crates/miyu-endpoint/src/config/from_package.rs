//! 核心拉起的扩展改配置（施工 T-9 补）：从 `set.rs` 挪出来（行数门禁，施工 Y-2 下下中上）。

use serde_json::Value as Json;

use miyu_config::Value;
use miyu_kernel::id::CommandId;
use miyu_kernel::origin::By;

use super::set::records::wanted;
use super::set::{ChangeParam, SetParams, set_by};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 核心拉起的扩展改配置（施工 T-9 补，接入QQ 的会话 2026-10-17 要的、项目主人定）：只收系统配置里、键以自己的包编号
/// `package` 开头、是这个包清单里声明的几项（施工 5-19），逐项改（不收整份换）；密钥类的项、写成引用（`{ secret = … }`、`{ env = … }`）的值不收：不然能把
/// 自己的项指到别人的密钥、环境里的 key 再读回去。值照这一项的类型读出来以后，任何一层（列表的元素、一组字段的字段）写成
/// 引用的都不收（施工 R3S-1：PC-2 的一组字段的列表里的密钥字段、网址列表里的 `{ env = … }` 曾经漏过）。不合的照旧
/// `local_only`。改的人记成这个包（`by`）。
pub(crate) fn set_from_package(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: SetParams,
    (package, by): (&str, By),
) -> Result<Json, Refusal> {
    let config = core.config();
    // 只认这个包清单里声明的项（施工 5-19：照挂在哪个包认，不照键的第一段；PN-1 起键照前缀起，和包编号不一定一样）。
    let own = |change: &ChangeParam| {
        miyu_config::key::item_of(config.items(), &change.key).is_some_and(|item| {
            crate::extensions::config::declared_by(item, package)
                && !holds_secret(&item.kind)
                && !wanted(item.kind, change.value.as_ref(), change.input.as_deref())
                    .is_some_and(|value| refers(&value))
        }) && !change.value.as_ref().is_some_and(Json::is_object)
            && !change
                .input
                .as_deref()
                .is_some_and(|input| input.trim_start().starts_with('{'))
    };
    let allowed = params.text.is_none()
        && params
            .changes
            .as_ref()
            .is_some_and(|changes| !changes.is_empty() && changes.iter().all(own));
    drop(config);
    if !allowed {
        return Err(Refusal::LOCAL_ONLY);
    }
    set_by(core, peer, cause, params, by)
}

/// 值里有没有写成引用的（`{ secret = … }`、`{ env = … }`）：列表的元素、一组字段的字段一层层看（施工 R3S-1）。
fn refers(value: &Value) -> bool {
    match value {
        Value::Secret(_) => true,
        Value::List(items) => items.iter().any(refers),
        Value::Record(fields) => fields.values().any(refers),
        Value::Text(_) | Value::Bool(_) | Value::Int(_) | Value::Float(_) => false,
    }
}

/// 这一类的值是不是密钥：密钥本身，或者密钥的列表。
fn holds_secret(kind: &miyu_config::Kind) -> bool {
    match kind {
        miyu_config::Kind::Secret => true,
        miyu_config::Kind::List(inner) => holds_secret(inner),
        _ => false,
    }
}

//! 自己的平台身份（施工 BD-1，项目主人 10-27 定）：每个人在个人设置里写 `external.me = ["qq:12345", "tg:7890123456"]`，说「这几个号
//! 是我」（一格列表，设置页一行一个，接入通讯平台的会话 f4 提的）。只能指自己（没有写别的账号的格），只有本人改得了（个人设置
//! 本来就只有他自己能写），号写错了是他自己的事（核心不查号在不在、写法对不对）。
//! 平台照头上的连接写在号的前面（`qq:`、`tg:`），核心照前缀认。
//!
//! 只在他自己名下的场所会话里认：他那一份 im 问 `venue.binding` 认得出他，他群里这个号说的话记成他本人（`session.send` 的 `as`）。
//! 主人的系统对应表照旧；平台桥（系统账号）名下的会话不认任何人的这张表（平台桥的配置是机主那一份，认了会把机主自己写的号
//! 记成平台桥本人）。

use std::collections::BTreeSet;

use miyu_kernel::id::{AccountId, ExternalId};

use crate::Core;

/// 个人设置里的那一格（`settings::EXTERNAL_ME`）。
const ME: &str = "external.me";

/// `owner` 在个人设置里说是自己的平台身份；平台桥（系统账号）的一个都没有。
pub(crate) fn mine(core: &Core, owner: &AccountId) -> BTreeSet<String> {
    if core.is_system(owner) {
        return BTreeSet::new();
    }
    let values = core.mine(owner).config().resolved().values();
    match values.get(ME) {
        Some(miyu_config::Value::List(ids)) => ids
            .iter()
            .filter_map(|id| match id {
                miyu_config::Value::Text(id) => Some(id.to_string()),
                _ => None,
            })
            .collect(),
        _ => BTreeSet::new(),
    }
}

/// `owner` 说过平台身份 `id` 是自己。
pub(crate) fn claims(core: &Core, owner: &AccountId, id: &ExternalId) -> bool {
    mine(core, owner).contains(id.as_str())
}

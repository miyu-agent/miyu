//! 核心拉起的扩展改配置（施工 T-9 补）：从 `set.rs` 挪出来（行数门禁，施工 Y-2 下下中上）。

use serde_json::Value as Json;

use miyu_kernel::id::CommandId;
use miyu_kernel::origin::By;

use super::set::{ChangeParam, SetParams, WriteLayer, set_by};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 核心拉起的扩展改配置（施工 T-9 补，接入QQ 的会话 2026-10-17 要的、项目主人定）：只收系统配置里、键以自己的包编号
/// `package` 开头、是这个包清单里声明的几项（施工 5-19），逐项改（不收整份换）；密钥类的项、写成引用（`{ secret = … }`、`{ env = … }`）的值不收：不然能把
/// 自己的项指到别人的密钥、环境里的 key 再读回去。不合的照旧 `local_only`。改的人记成这个包（`by`）。
pub(crate) fn set_from_package(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: SetParams,
    (package, by): (&str, By),
) -> Result<Json, Refusal> {
    let config = core.config();
    let prefix = format!("{package}.");
    // 键以包编号开头还不够：包编号和核心模块同名的包（叫 `ui`、`sandbox` 的）没声明配置项也装得上，`ui.*` 却是核心自己的项
    // （施工 5-19）。只认这个包清单里声明的项（`ui.package` 是它）。
    let own = |change: &ChangeParam| {
        change.key.starts_with(&prefix)
            && miyu_config::key::item_of(config.items(), &change.key)
                .is_some_and(|item| item.ui.package == Some(package) && !holds_secret(&item.kind))
            && !change.value.as_ref().is_some_and(Json::is_object)
            && !change
                .input
                .as_deref()
                .is_some_and(|input| input.trim_start().starts_with('{'))
    };
    let allowed = matches!(params.layer, WriteLayer::System)
        && params.text.is_none()
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

/// 这一类的值是不是密钥：密钥本身，或者密钥的列表。
fn holds_secret(kind: &miyu_config::Kind) -> bool {
    match kind {
        miyu_config::Kind::Secret => true,
        miyu_config::Kind::List(inner) => holds_secret(inner),
        _ => false,
    }
}

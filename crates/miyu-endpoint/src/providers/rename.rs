//! `provider.rename`（施工 8-34，`docs/blueprint/models.md`「供应商改名」）：改一家供应商的编号，一次改完。
//!
//! 1. `to` 照编号的写法查，和 `from` 一样的、写法不对的 `bad_params`；合出来的配置里没有 `from` 的 `unknown_provider`，已经有
//!    `to` 的 `provider_exists`。
//! 2. 先个人设置、再系统配置，每一层照 `config.set` 的规矩写（`config/rewrite.rs`）：有 `[providers.<from>]` 的那一层换表名
//!    （子表跟着走，密钥的引用在表里，跟着走），两层里指着 `<from>/<模型>` 的引用、模型的列表都换成 `<to>/<模型>`。后一层
//!    写不成的，前一层改了的不撤，回那一层的错；再调一次照剩下的改。
//! 3. 用出来的、供应商的列表换编号，改名记进 `state/models/renamed.json`：钉着旧编号的会话下一轮起照新编号
//!    （`miyu-session` 的 `route/turn.rs`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::edit::{self, Change};
use miyu_config::parse::parse;
use miyu_config::rename::{rename_table, retargeted};
use miyu_config::{Item, Layer};
use miyu_kernel::id::CommandId;
use miyu_models::provider::configured;

use crate::Core;
use crate::config::rewrite::{Unwritten, rewrite};
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `provider.rename` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameParams {
    from: String,
    to: String,
}

/// `provider.rename`：回 `{}`，改了的照 `config.changed` 推。
pub(crate) async fn rename(
    core: &Arc<Core>,
    peer: Peer,
    cause: &CommandId,
    params: Value,
) -> Result<Value, Refusal> {
    let RenameParams { from, to } =
        serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    if from == to || !miyu_config::key::valid(miyu_config::key::ID, &to) {
        return Err(Refusal::BAD_PARAMS);
    }
    let names = configured(&core.config().resolved().values());
    if !names.contains(&from) {
        return Err(Refusal::UNKNOWN_PROVIDER);
    }
    if names.contains(&to) {
        return Err(Refusal::PROVIDER_EXISTS);
    }
    let items = core.config().items().to_vec();
    rewrite(core, peer, cause, Layer::System, |text| {
        renamed(&items, Layer::System, text, &from, &to)
    })?;
    let data = Arc::clone(&core.model_data);
    tokio::task::spawn_blocking(move || data.rename_provider(&from, &to))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    Ok(json!({}))
}

/// 一层的字改名以后的样子：没有要改的交回 `None`。
fn renamed(
    items: &[Item],
    layer: Layer,
    text: &str,
    from: &str,
    to: &str,
) -> Result<Option<String>, Unwritten> {
    let table = rename_table(text, "providers", from, to).map_err(|_| Unwritten::Blocked)?;
    let mut edited = table.clone().unwrap_or_else(|| text.to_string());
    let parsed = parse(items, layer, &edited).map_err(|_| Unwritten::Blocked)?;
    let changes = retargeted(items, &parsed, from, to);
    if table.is_none() && changes.is_empty() {
        return Ok(None);
    }
    for (key, value) in &changes {
        edited = edit::apply(&edited, Change::Set(key, value)).map_err(|_| Unwritten::Blocked)?;
    }
    Ok(Some(edited))
}

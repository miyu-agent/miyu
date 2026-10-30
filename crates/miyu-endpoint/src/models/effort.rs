//! `session.configure` 的 `effort`（施工 SM-1，设计 51，`docs/blueprint/models.md`「协议」）：要换成的一档照会话这时的模型的
//! 档位查，不在里面的 `bad_params`（`data.field` 是 `effort`），什么都不记；`null` 是回到跟着配置，不查。
//!
//! 「会话这时的模型」：这一条一起换了模型的是换成的那个（回到跟着配置的照这一刻的 `models.chat`），没换的是会话记着的引用
//! （`Handle::current` 的 `model`，SM-1 补），没记的照 `models.chat`。池一档都没有，设什么都 `bad_params`（SM-1 补，项目
//! 主人 10-30：「池子不应该能设置思考强度」）：池里每个成员照它自己的配置。

use miyu_models::effort;

use crate::Core;
use crate::refusal::Refusal;

/// 查要换成的一档 `level`：`reference` 是会话这时的引用（模型或 `@池`），没有的一档都没有。交回规整过的名字（`none`、
/// `disabled` 读成 `off`）；`level` 没有的交回没有。
///
/// # Errors
///
/// 空字、不在档位里：`bad_params`（`data.field` 是 `effort`，原话记一行 `DEBUG unknown effort`）。
pub(crate) async fn checked(
    core: &Core,
    reference: Option<&str>,
    level: Option<&str>,
) -> Result<Option<String>, Refusal> {
    let Some(level) = level else {
        return Ok(None);
    };
    let level = effort::normalize(level).to_string();
    core.model_data.wait().await;
    let resolved = core.config().resolved().clone();
    let levels = match reference {
        Some(text) => core
            .model_data
            .with(|knowledge| effort::levels_of(&resolved, knowledge, text)),
        None => Vec::new(),
    };
    if level.is_empty() || !levels.contains(&level) {
        tracing::debug!(target: "miyu::endpoint", model = reference.unwrap_or_default(), level = level.as_str(), "unknown effort");
        return Err(Refusal::bad_field("effort"));
    }
    Ok(Some(level))
}

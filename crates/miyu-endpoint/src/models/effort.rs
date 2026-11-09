//! `session.configure` 的 `effort`（施工 SM-1，设计 51，`docs/blueprint/models.md`「协议」）：要换成的一档照会话这时的模型的
//! 档位查，不在里面的 `bad_params`（`data.field` 是 `effort`），什么都不记；`null` 是回到跟着配置，不查。
//!
//! 「会话这时的模型」：这一条一起换了模型的是换成的那个（回到跟着配置的照这一刻的 `models.chat`），没换的是会话记着的引用
//! （`Handle::current` 的 `model`，SM-1 补），没记的照 `models.chat`。池一档都没有，设什么都 `bad_params`（SM-1 补，项目
//! 主人 10-30：「池子不应该能设置思考强度」）：池里每个成员照它自己的配置。

use miyu_kernel::session::Command;
use miyu_models::effort;
use miyu_session::Handle;

use crate::Core;
use crate::refusal::Refusal;

/// `session.configure` 的 `effort` 要交给内核的命令：照会话这时的模型查要换成的一档 `level`。`model` 是这一条一起换成的引用
/// （空的是回到跟着 `models.chat`），没换的没有。
///
/// 会话这时的模型：一起换了的是换成的，回到跟着配置的是这一刻的 `models.chat`（施工 SM-1）；没换的是内核记着的（SM-1 补：刚换
/// 到池、还没开回合的，路由上还是上一轮的模型，照它查会放过池），没记的跟着 `models.chat`。
///
/// # Errors
///
/// 同 [`checked`]。
pub(crate) async fn command(
    core: &Core,
    handle: &Handle,
    model: Option<&str>,
    level: Option<&str>,
) -> Result<Command, Refusal> {
    let chat = || miyu_models::provider::chat(&core.config().resolved().values());
    let reference = match model {
        Some("") => chat(),
        Some(model) => Some(model.to_string()),
        None => match handle.current().await {
            Ok(current) => current.model.or_else(chat),
            Err(_) => handle.next().reference,
        },
    };
    let level = checked(core, reference.as_deref(), level).await?;
    Ok(Command::SetEffort { level })
}

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
    // 临时问一句（审计 R3E-7）：引用只查过供应商配没配，模型名随便写都过，被拒的这一条问过的不该记进 `catalog.json`。
    let resolved = core.config().resolved().clone();
    let levels = match reference {
        Some(text) => core
            .model_data
            .with_once(|knowledge| effort::levels_of(&resolved, knowledge, text)),
        None => Vec::new(),
    };
    if level.is_empty() || !levels.contains(&level) {
        tracing::debug!(target: "miyu::endpoint", model = reference.unwrap_or_default(), level = level.as_str(), "unknown effort");
        return Err(Refusal::bad_field("effort"));
    }
    Ok(Some(level))
}

//! 人卡的协议（施工 E-1 下，`docs/blueprint/memory.md`「协议」、第七条第 15 款）：`memory.cards` 列、`memory.card_update` 人写一张、
//! `memory.card_forget` 作废一张。找哪一间同 `memory.*`；怎么查、怎么写在会话那一层的 [`miyu_session::Keeper`]。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_recall::merge::CARD_CHARS;
use miyu_recall::{Card, PersonCard};
use miyu_session::{CardRefused, Stamp};

use super::params::Where;
use super::{find, memory_failed};
use crate::Core;
use crate::refusal::Refusal;

/// `memory.cards`。
#[derive(Debug, Deserialize)]
pub(super) struct Cards {
    #[serde(flatten)]
    at: Where,
    /// 连作废的一起。
    #[serde(default)]
    forgotten: bool,
}

/// `memory.card_update`：`text` 要写，`names` 不写的是没有叫法。
#[derive(Debug, Deserialize)]
pub(super) struct CardUpdate {
    #[serde(flatten)]
    at: Where,
    who: String,
    text: String,
    #[serde(default)]
    names: Vec<String>,
}

/// `memory.card_forget`。
#[derive(Debug, Deserialize)]
pub(super) struct CardForget {
    #[serde(flatten)]
    at: Where,
    who: String,
    #[serde(default)]
    why: Option<String>,
}

/// 列这一间的人卡。
pub(super) async fn cards(
    core: &Arc<Core>,
    account: &AccountId,
    params: Cards,
) -> Result<Value, Refusal> {
    let keeper = find(core, account, params.at).await?;
    let forgotten = params.forgotten;
    let found = tokio::task::spawn_blocking(move || keeper.cards(forgotten))
        .await
        .map_err(|error| memory_failed(error.is_panic()))?
        .map_err(|error| {
            tracing::warn!(target: "miyu::endpoint", error = error.as_str(), "memory failed");
            Refusal::INTERNAL
        })?;
    Ok(json!({"cards": found.iter().map(shown).collect::<Vec<_>>()}))
}

/// 人写一张卡：正文超过 [`CARD_CHARS`] 字的太长，别的不合规矩的参数不对。
pub(super) async fn update(
    core: &Arc<Core>,
    account: &AccountId,
    stamp: Stamp,
    params: CardUpdate,
) -> Result<Value, Refusal> {
    let chars = params.text.trim().chars().count();
    if chars > CARD_CHARS {
        return Err(Refusal::memory_too_long(chars, CARD_CHARS, "text"));
    }
    let keeper = find(core, account, params.at).await?;
    let card = Card {
        who: params.who,
        text: params.text,
        names: params.names,
    };
    run(move || keeper.write_card(stamp, card)).await?;
    Ok(json!({}))
}

/// 人作废一张卡：没有这个人的卡 `unknown_memory`。
pub(super) async fn forget(
    core: &Arc<Core>,
    account: &AccountId,
    stamp: Stamp,
    params: CardForget,
) -> Result<Value, Refusal> {
    let keeper = find(core, account, params.at).await?;
    let (who, why) = (params.who, params.why.unwrap_or_default());
    run(move || keeper.retire_card(stamp, who.trim(), why)).await?;
    Ok(json!({}))
}

/// 在阻塞线程里写，[`CardRefused`] 写成拒绝。
async fn run(
    work: impl FnOnce() -> Result<(), CardRefused> + Send + 'static,
) -> Result<(), Refusal> {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(CardRefused::Bad)) => Err(Refusal::BAD_PARAMS),
        Ok(Err(CardRefused::NoSuch)) => Err(Refusal::UNKNOWN_MEMORY),
        Ok(Err(CardRefused::Failed(refused))) => {
            tracing::warn!(target: "miyu::endpoint", error = ?refused, "memory failed");
            Err(Refusal::INTERNAL)
        }
        Err(error) => Err(memory_failed(error.is_panic())),
    }
}

/// 一张卡在回应里的样子。
fn shown(card: &PersonCard) -> Value {
    let by = serde_json::to_value(&card.by)
        .ok()
        .and_then(|by| by.get("kind").cloned())
        .unwrap_or(Value::Null);
    json!({
        "who": card.who,
        "text": card.text,
        "names": card.names,
        "at": card.at.to_string(),
        "by": by,
        "retired": card.retired,
    })
}

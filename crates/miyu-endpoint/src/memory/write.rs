//! 人经协议记一条、改一条（`memory.remember`、`memory.update`，施工 R-3 补；R-15 上加关键词，E-1 上加认识的人的 `who`；从
//! `memory.rs` 挪出来）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_recall::{CLASSES, PERSON};
use miyu_session::Stamp;
use miyu_tool::Remember;

use super::checks::{checked, keywords};
use super::params::{self, Update};
use super::{blocking, fill, find, memory_id};
use crate::Core;
use crate::refusal::Refusal;

pub(super) async fn remember(
    core: &Arc<Core>,
    account: &AccountId,
    stamp: Stamp,
    params: params::Remember,
) -> Result<Value, Refusal> {
    if !CLASSES.contains(&params.class.as_str()) {
        return Err(Refusal::BAD_PARAMS);
    }
    let keywords = params.keywords.as_deref().map(keywords).transpose()?;
    let who = params
        .who
        .map(|who| who.trim().to_string())
        .filter(|who| !who.is_empty());
    // 认识的人那一类要说是谁（施工 E-1 上）；别的类写了也不看。
    if params.class == PERSON && who.is_none() {
        return Err(Refusal::BAD_PARAMS);
    }
    let remember = Remember {
        who: who.filter(|_| params.class == PERSON),
        class: params.class,
        text: checked(&params.text)?,
        replaces: None,
        keywords: keywords.filter(|keywords| !keywords.is_empty()),
    };
    let keeper = find(core, account, params.at).await?;
    let id = blocking({
        let keeper = keeper.clone();
        move || keeper.save(stamp, remember, Vec::new())
    })
    .await?;
    fill(core, keeper).await;
    Ok(json!({"id": id.to_string()}))
}

pub(super) async fn update(
    core: &Arc<Core>,
    account: &AccountId,
    stamp: Stamp,
    params: Update,
) -> Result<Value, Refusal> {
    let id = memory_id(&params.id)?;
    let text = params.text.as_deref().map(checked).transpose()?;
    let keywords = params.keywords.as_deref().map(keywords).transpose()?;
    let who = params
        .who
        .map(|who| who.trim().to_string())
        .filter(|who| !who.is_empty());
    if text.is_none() && keywords.is_none() && who.is_none() {
        return Err(Refusal::BAD_PARAMS);
    }
    let keeper = find(core, account, params.at).await?;
    let id = blocking({
        let keeper = keeper.clone();
        move || keeper.update(stamp, id, text, keywords, who)
    })
    .await?;
    fill(core, keeper).await;
    Ok(json!({"id": id.to_string()}))
}

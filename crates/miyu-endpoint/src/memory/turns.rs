//! 搜以前的对话（施工 R-12 下，`docs/blueprint/memory.md`「协议」，`protocol.md` 的 `memory.turns`）：查这一间的回合索引，
//! 同她的 `memory_search` 搜以前的对话那一路，只是人看得见所有的会话，不跳过哪一个。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_tool::Refused;

use super::params::Turns;
use super::{SEARCH, blocking, find, limit, using};
use crate::Core;
use crate::refusal::Refusal;

/// `memory.turns`：照关键词找，有照意思那一路的再照意思找（问句的向量照这时的配置算，搜完在后台补这一间缺的，同
/// `memory.search`），最相关的在前。
pub(super) async fn turns(core: &Arc<Core>, params: Turns) -> Result<Value, Refusal> {
    if params.query.trim().is_empty() {
        return Err(Refusal::BAD_PARAMS);
    }
    let limit = limit(params.limit, SEARCH)?;
    let keeper = find(core, params.at).await?;
    let query = params.query;
    let using = using(core);
    let near = match keeper.vectors() {
        Some(vectors) => vectors.query(&using, &query).await,
        None => None,
    };
    let searched = keeper.clone();
    let found = blocking(move || {
        searched
            .turns(&query, limit, (None, false), near.as_ref())
            .map_err(Refused::Failed)
    })
    .await?;
    if keeper.vectors().is_some() {
        blocking(move || {
            keeper.fill(&using);
            Ok(())
        })
        .await?;
    }
    let turns: Vec<Value> = found
        .iter()
        .map(|turn| {
            json!({
                "session": turn.session.to_string(),
                "turn": turn.turn,
                "at": turn.at.to_string(),
                "text": turn.text,
            })
        })
        .collect();
    Ok(json!({ "turns": turns }))
}

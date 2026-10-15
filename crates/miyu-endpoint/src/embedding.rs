//! `model.embed {texts}`（施工 X-17，`docs/designs/34-表情包.md` 第五节第 2 条）：照这个账号的 `models.embedding`（本机的、远程
//! 那一家）算一批字的向量，交回模型编号和向量；换了模型的向量不能和以前的比，扩展照编号重算。远程的用量记在管理员名下，同记忆
//! 照意思找（`memory::using`）。和 `model.call` 一样不另设能力。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_session::Unavailable;

use crate::Core;
use crate::refusal::Refusal;

/// 一批至多几条。
const MOST_TEXTS: usize = 64;

/// 一条至多几个字。
const MOST_CHARS: usize = 8000;

/// `model.embed` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmbedParams {
    texts: Vec<String>,
}

/// `model.embed`：见模块的说明。
pub(crate) async fn embed(core: &Arc<Core>, params: EmbedParams) -> Result<Value, Refusal> {
    let fits = |text: &String| (1..=MOST_CHARS).contains(&text.chars().count());
    if !(1..=MOST_TEXTS).contains(&params.texts.len()) || !params.texts.iter().all(fits) {
        return Err(Refusal::BAD_PARAMS);
    }
    let Some(vectors) = core.memory.vectors() else {
        return Err(Refusal::EMBEDDING_OFF);
    };
    let using = crate::memory::using(core);
    match vectors.embed_all(&using, &params.texts).await {
        Ok((model, vectors)) => Ok(json!({"model": model, "vectors": vectors})),
        Err(Unavailable::Off(_)) => Err(Refusal::EMBEDDING_OFF),
        Err(Unavailable::Failed(why)) => Err(Refusal::embedding_failed(&why)),
    }
}

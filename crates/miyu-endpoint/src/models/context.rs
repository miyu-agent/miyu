//! 会话的上下文上限（施工 CTX-1、CTX-2，设计 68 第四节）：`session.configure` 的 `context` 和 `/context` 共用的查法。
//!
//! 设得太小、照它算不出压缩线的拒（`bad_params`，`data.field` 是 `context`，`data.min` 是最少要多少）：最少要多少由内核照这时
//! 的策略和端口交的最大输出算（`Session::least_context`，和压缩线同一个算法），什么都不记。策略里没有压缩的不主动压，设多少
//! 都行，只拒 0。`null` 是去掉，不查。

use serde_json::json;

use miyu_kernel::session::Command;
use miyu_session::Handle;

use crate::refusal::Refusal;

/// 要交给内核的命令：`tokens` 是要设的上限（`None` 是去掉），先照会话这一刻的压缩线查过。
///
/// # Errors
///
/// 比最少要的还小（或者是 0）：`bad_params`，`data.field` 是 `context`，`data.min` 是最少要多少。会话停了：`stopped`。
pub(crate) async fn command(handle: &Handle, tokens: Option<u64>) -> Result<Command, Refusal> {
    let Some(tokens) = tokens else {
        return Ok(Command::SetContext { tokens: None });
    };
    let least = handle
        .current()
        .await
        .map_err(|_| Refusal::STOPPED)?
        .least_context
        .unwrap_or(1)
        .max(1);
    if tokens < least {
        return Err(too_small(least));
    }
    Ok(Command::SetContext {
        tokens: Some(tokens),
    })
}

/// 太小：`data.field` 是 `context`，`data.min` 是最少要多少。
fn too_small(least: u64) -> Refusal {
    let mut refusal = Refusal::bad_field("context");
    refusal
        .data
        .get_or_insert_with(serde_json::Map::new)
        .insert("min".to_string(), json!(least));
    refusal
}

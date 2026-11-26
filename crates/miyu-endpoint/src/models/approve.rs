//! 扩展驱动在一次 `driver.call` 进行中请核心问人（施工 ACP-8，设计 73 第四节第 7 条，蓝图 `extensions.md`「驱动要人确认」）：
//! `driver.approve` 照 `call` 交给这个连接上在进行的那一次，执行器、会话问完了回 `result`。不是这个连接上在进行的那一次的回
//! `unknown_call`；参数写得不对的回 `bad_params`（`data.why` 是一句英文）；问着的时候那一次没了（说完了、被打断、超时、会话
//! 停了）回 `{"decision": "cancelled"}`。在后台等，不挡这个连接上别的请求。

use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::refusal::{Locale, Refusal};
use crate::reverse::Peer;
use crate::wire::{self, Request};

/// 在后台办一条 `driver.approve`：连接是 `peer`，等到了把回应照 `locale` 写进 `out`。连接断了的写不进，不用说。
pub(crate) async fn later(
    (peer, request, locale, out): (Peer, Request, Locale, mpsc::Sender<String>),
) {
    let line = match approve(&peer, &request).await {
        Ok(result) => wire::result(&request.id, result),
        Err(refusal) => wire::error(
            Value::String(request.id.as_str().to_string()),
            refusal,
            locale,
        ),
    };
    if out.send(line).await.is_err() {
        // 连接断了：驱动那一头也没了。
    }
}

/// 办一条 `driver.approve`：连接是 `peer`。交回回应的 `result` 或者拒绝。
pub(crate) async fn approve(peer: &Peer, request: &Request) -> Result<Value, Refusal> {
    let call = request
        .params
        .get("call")
        .and_then(Value::as_str)
        .ok_or(Refusal::BAD_PARAMS)?;
    let answered = peer
        .approve(call, request.params.clone())
        .ok_or(Refusal::UNKNOWN_CALL)?;
    match answered.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(why)) => Err(Refusal::BAD_PARAMS.because(why)),
        Err(_) => Ok(json!({"decision": "cancelled"})),
    }
}

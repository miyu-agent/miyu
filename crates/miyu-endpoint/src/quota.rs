//! `session.quota`（施工 QU-1，`docs/blueprint/protocol.md`「`session.quota`」）：这个会话接下来请求的那一家的额度，几个窗口
//! 各用了百分之几、什么时候重置。只有档案写了查额度接口的供应商有（现在是 opencode Go），别的、轮换的池、查不到的回 `{}`。
//!
//! 查和隔多久查在 `miyu-session` 的 `route/quota.rs`：同一家 60 秒内照上一次的，会话每轮结束在后台查过一次，头开用量框时多半
//! 不用等。窗口的名字照连接的语言（`core/human/<语言>.json` 的 `quota/window/<编号>`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_kernel::id::SessionId;
use miyu_models::quota::Window;

use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Params {
    session: String,
}

/// `{"windows": [{id, label, percent, status, resets_at}…]}`；没有的 `{}`。
pub(crate) async fn quota(core: &Arc<Core>, peer: Peer, params: Params) -> Result<Value, Refusal> {
    let id = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &id).await?;
    let Some(model) = found.handle.next().model else {
        return Ok(json!({}));
    };
    let data = Arc::clone(&core.model_data);
    data.wait().await;
    let snapshot = crate::models::snapshot(core);
    let secret = |reference: &_| snapshot.secret(reference);
    let windows = miyu_session::quota(
        &data,
        &snapshot.resolved.values(),
        &secret,
        model.endpoint.as_str(),
    )
    .await;
    let Some(windows) = windows else {
        return Ok(json!({}));
    };
    let human = words(core, peer.language)?;
    Ok(json!({"windows": shown(&windows, &human)}))
}

/// 照协议写：名字照 `human` 的语言。
fn shown(windows: &[Window], human: &impl Words) -> Vec<Value> {
    windows
        .iter()
        .map(|window| {
            let id = window.id.as_str();
            let label = human
                .sentence(&format!("quota/window/{id}"), &[])
                .unwrap_or_else(|| id.to_string());
            json!({
                "id": id,
                "label": label,
                "percent": window.percent,
                "status": window.status,
                "resets_at": window.resets_at,
            })
        })
        .collect()
}

//! `session.quota`（施工 QU-1，`docs/blueprint/protocol.md`「`session.quota`」）：这个会话接下来请求的那一家的额度，几个窗口
//! 各用了百分之几、什么时候重置（`windows`，opencode Go），或者账户余额（`balance`，施工 QU-2，DeepSeek 官方）。只有档案写了
//! 查额度接口的供应商有，两格各是各的，没查到的那一格不带；别的、轮换的池、查不到的回 `{}`。
//!
//! 「接下来请求的那一家」照会话记着的引用（施工 QU-2修，[`upcoming`]）：刚换了模型、还没开回合的，路由上还是上一轮解析的那一家
//! （`subscribe` 的 `model` 也是，下一个回合开始才换），照它查就成了换之前的那一家。
//!
//! 查和隔多久查在 `miyu-session` 的 `route/quota.rs`：同一家 60 秒内照上一次的，会话每轮结束在后台查过一次，头开用量框时多半
//! 不用等。窗口的名字照连接的语言（`core/human/<语言>.json` 的 `quota/window/<编号>`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_config::{Values, Words};
use miyu_kernel::id::SessionId;
use miyu_models::provider;
use miyu_models::quota::{Balance, Window};
use miyu_models::reference::Reference;
use miyu_session::Handle;

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

/// `{"windows": [{id, label, percent, status, resets_at}…], "balance": {available, items: [{currency, total, granted,
/// topped_up}…]}}`，没查到的那一格不带；都没有的 `{}`。
pub(crate) async fn quota(core: &Arc<Core>, peer: Peer, params: Params) -> Result<Value, Refusal> {
    let id = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &id).await?;
    let data = Arc::clone(&core.model_data);
    data.wait().await;
    let snapshot = crate::models::snapshot(core);
    let values = snapshot.resolved.values();
    let Some(endpoint) = upcoming(core, &found.handle, &values).await else {
        return Ok(json!({}));
    };
    let secret = |reference: &_| snapshot.secret(reference);
    let reading = miyu_session::quota(&data, &values, &secret, &endpoint).await;
    let mut answer = Map::new();
    let Some(reading) = reading else {
        return Ok(Value::Object(answer));
    };
    if !reading.windows.is_empty() {
        let human = words(core, peer.language)?;
        answer.insert("windows".into(), shown(&reading.windows, &human).into());
    }
    if let Some(balance) = &reading.balance {
        answer.insert("balance".into(), balanced(balance));
    }
    Ok(Value::Object(answer))
}

/// 会话接下来请求的那一家的编号（施工 QU-2修），照这一刻的最终值 `values`：会话记着的引用（内核照日志算的，刚换了、还没开回合
/// 的也是换成的；没记的跟着 `models.chat`，同 `models/effort.rs`）和路由上一次解析的一样的，照路由解析出的那一家（钉住的池照
/// 钉着的成员）；不一样的是换过了、下一个回合开始才重新解析，照引用写的那一家（配置里没有、改过名的照新编号，同
/// `route/turn.rs`）。换成了池的这时还不知道钉哪个成员，没有；轮换的池、一个都没有的也没有。会话停了的照路由的。
async fn upcoming(core: &Core, handle: &Handle, values: &Values) -> Option<String> {
    let next = handle.next();
    let routed = next.model.map(|model| model.endpoint.as_str().to_string());
    let Ok(current) = handle.current().await else {
        return routed;
    };
    let recorded = current.model.or_else(|| provider::chat(values));
    if recorded == next.reference {
        return routed;
    }
    match Reference::parse(recorded.as_deref()?).ok()? {
        Reference::Model { provider: id, .. } if !provider::configured(values).contains(&id) => {
            core.model_data.renamed(&id).or(Some(id))
        }
        Reference::Model { provider: id, .. } => Some(id),
        Reference::Pool(_) => None,
    }
}

/// 余额照协议写：金额、币种照官方原样。
fn balanced(balance: &Balance) -> Value {
    let items: Vec<Value> = balance
        .items
        .iter()
        .map(|item| {
            json!({
                "currency": item.currency,
                "total": item.total,
                "granted": item.granted,
                "topped_up": item.topped_up,
            })
        })
        .collect();
    json!({"available": balance.available, "items": items})
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

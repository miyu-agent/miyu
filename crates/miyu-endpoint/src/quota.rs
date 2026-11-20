//! `session.quota`（施工 QU-1，`internal/blueprint/protocol.md`「`session.quota`」）：这个会话接下来请求的那一家的额度，几个窗口
//! 各用了百分之几、什么时候重置（`windows`，opencode Go），或者账户余额（`balance`，施工 QU-2，DeepSeek 官方）。只有档案写了
//! 查额度接口的供应商有，两格各是各的，没查到的那一格不带；别的、轮换的池、查不到的回 `{}`。
//!
//! 「接下来请求的那一家」照 `subscribe` 的 `model.endpoint`（`Handle::next`）：刚换了模型、还没开回合的，那里已经是照会话记着的
//! 引用预演的（施工 MC-1，收了 QU-2修 在这里另算的一份；`miyu-session` 的 `route/upcoming.rs`），换成池的下一轮钉了成员才有。
//!
//! 查和隔多久查在 `miyu-session` 的 `route/quota.rs`：同一家 60 秒内照上一次的，会话每轮结束在后台查过一次，头开用量框时多半
//! 不用等。窗口的名字照连接的语言（`core/human/<语言>.json` 的 `quota/window/<编号>`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_config::Values;
use miyu_config::Words;
use miyu_kernel::id::SessionId;
use miyu_models::provider;
use miyu_models::quota::{Balance, Window};
use miyu_models::settings::ProviderSettings;
use miyu_session::ModelData;

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

/// `{"provider": {id, name}, "windows": [{id, label, percent, status, resets_at}…], "balance": {available, items: [{currency,
/// total, granted, topped_up}…]}}`，没查到的那一格不带；都没有的 `{}`（也不带 `provider`）。
pub(crate) async fn quota(core: &Arc<Core>, peer: Peer, params: Params) -> Result<Value, Refusal> {
    let id = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &id).await?;
    let data = Arc::clone(&core.model_data);
    data.wait().await;
    let snapshot = crate::models::snapshot(core);
    let values = snapshot.resolved.values();
    let Some(model) = found.handle.next().model else {
        return Ok(json!({}));
    };
    let secret = |reference: &_| snapshot.secret(reference);
    let reading = miyu_session::quota(&data, &values, &secret, model.endpoint.as_str()).await;
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
    // 查的是哪一家（施工 QU-3）：头照它写「{供应商} 额度情况」，不照自己记的模型猜。
    if !answer.is_empty() {
        let id = model.endpoint.as_str();
        let name = provider_name(&data, &values, id);
        answer.insert("provider".into(), json!({"id": id, "name": name}));
    }
    Ok(Value::Object(answer))
}

/// 这一家给人看的名字（同 `model.list` 的 `name`，施工 8-21）：配置写了的（去掉两头空白不是空的）、目录里对上的那一家的、编号。
fn provider_name(data: &ModelData, values: &Values, id: &str) -> String {
    let settings = ProviderSettings::at(values, &[id]);
    if let Some(name) = settings
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return name.to_string();
    }
    data.with_once(|knowledge| {
        let found = provider::provider(values, knowledge, id).ok()?;
        let recognized = found.recognized?;
        knowledge
            .lookup?
            .listing(&recognized.provider)?
            .name
            .clone()
    })
    .unwrap_or_else(|| id.to_string())
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

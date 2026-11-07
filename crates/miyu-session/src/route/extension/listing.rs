//! 扩展驱动的模型列表（施工 MD-4，设计 66 第四节第 4 条）：拉供应商的列表时（`route/lists.rs`）驱动是扩展的，不走 HTTP，经端口发
//! `driver.models`（可选），读回 `{"models": [{"id", "window"?, "max_output"?, "tools"?, "vision"?, "reasoning"?}]}`，交给列表那一层
//! 照常存、照常查：名字、窗口、最大输出、能不能调工具、看不看得了图都用上，思考照收不用（core1 11-07 照推荐定）。没实现的、连不上的、写坏了的照拉不到办（列表照旧用上一份，主人手写）。key、地址照挑候选时的规矩交
//! （[`super::reach`]）；不交增量，不发 `driver.cancel`。

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::secret::{Reference, Secret};
use miyu_models::observed::ListedModel;
use miyu_models::provider::{DriverRef, NoModel, Provider};

use super::super::driver_port::Broken;
use super::super::shared::ModelData;

/// `driver.models` 的回答。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    models: Vec<Entry>,
}

/// 列出来的一个模型。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    #[serde(default)]
    window: Option<u64>,
    #[serde(default)]
    max_output: Option<u64>,
    #[serde(default)]
    tools: Option<bool>,
    #[serde(default)]
    vision: Option<bool>,
    #[serde(default)]
    reasoning: Option<bool>,
}

/// 经扩展驱动 `driver` 拉这一家（`provider`）的模型列表，整个最多 `timeout`；`secret` 取 key、地址。
///
/// # Errors
///
/// 这一家用不了（驱动不在、key 取不到）、连不上、没答、扩展回了错误（没实现也是）、写坏了：英文的一句。
pub(in crate::route) async fn listed(
    data: &ModelData,
    provider: &Provider,
    driver: &DriverRef,
    secret: &(dyn Fn(&Reference) -> Option<Secret> + Sync),
    timeout: Duration,
) -> Result<Vec<ListedModel>, String> {
    let reach = super::reach(data, provider, driver, secret).map_err(|NoModel(why)| why)?;
    let port = data.drivers().ok_or("no channel to model drivers")?;
    let mut params = json!({"driver": driver.to_string()});
    if let Some(key) = &reach.key {
        params["key"] = json!(key.expose());
    }
    if let Some(base_url) = &reach.base_url {
        params["base_url"] = json!(base_url);
    }
    let pending = port
        .ask(driver, "driver.models", params.to_string())
        .map_err(|_| format!("model driver \"{driver}\" isn't connected yet"))?;
    match tokio::time::timeout(timeout, pending).await {
        Err(_) => Err("the driver did not list its models in time".to_string()),
        Ok(Err(Broken::Gone)) => Err("the driver went away".to_string()),
        Ok(Err(Broken::Refused(message))) => {
            Err(format!("the driver refused to list its models: {message}"))
        }
        Ok(Ok(result)) => models(result),
    }
}

/// 读回答：写坏了的整份不要。
fn models(result: Value) -> Result<Vec<ListedModel>, String> {
    if !result.is_object() {
        return Err("the model list is not an object".to_string());
    }
    let answer: Answer = serde_json::from_value(result)
        .map_err(|error| format!("the driver sent a bad model list: {error}"))?;
    Ok(answer
        .models
        .into_iter()
        .map(|entry| {
            // 思考照收不用（core1 11-07 定）：资料里要的是档位表，开关推不出档位。
            let _ = entry.reasoning;
            ListedModel {
                id: entry.id,
                window: entry.window,
                max_output: entry.max_output,
                tools: entry.tools,
                vision: entry.vision,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests;

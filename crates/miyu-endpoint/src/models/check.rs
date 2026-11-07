//! `driver.check`（施工 MD-3，设计 66 第七节第 2 条，core1 11-07 照推荐定）：查一个包提供的模型驱动编码稳不稳，作者用
//! `miyu dev driver-check <包编号>` 调它。只给人。
//!
//! - 照 `miyu_session::driver_check` 的样本调扩展的 `driver.encode`：每一份编两次；重启扩展、等它重新握手，再编一次；三次要逐字节相同。
//!   只在末尾加一条消息的一对，到记号字为止要相同，找不到记号字的报红。
//! - 要重启：这一刻有会话正经这个扩展的驱动调模型（有没答完的 `driver.call`）的不重启，回 `driver_busy`（`data.running`）；重启前再查
//!   一次。
//! - 没实现 `driver.encode`（回「没有这个方法」）的：声明 `cache = "contract"` 的算红，别的算跳过。

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::{DriverCache, DriverDecl};
use miyu_session::driver_check::{self, Sample};

use crate::Core;
use crate::hello::Peer as Caller;
use crate::refusal::Refusal;
use crate::reverse::{Gone, Peer};

/// 一次 `driver.encode` 最多等多久。
const ENCODE_WAIT: Duration = Duration::from_secs(30);

/// 重启以后最多等多久重新握手。
const RELINK_WAIT: Duration = Duration::from_secs(30);

/// JSON-RPC 的「没有这个方法」。
const METHOD_NOT_FOUND: i64 = -32601;

/// `driver.check` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckParams {
    package: String,
}

/// 一次编码怎么了：编出了字节、没实现、出错（英文的一句）。
enum Encoded {
    Body(Vec<u8>),
    Missing,
    Failed(String),
}

/// 一个驱动这一轮查到的：每一份样本过没过；没实现的。
struct Checked {
    decl: DriverDecl,
    name: String,
    samples: Vec<(&'static str, Option<String>)>,
    missing: bool,
}

/// `driver.check`：见模块的说明。
///
/// # Errors
///
/// 没装、不是扩展（`unknown_package`）；没声明驱动（`no_drivers`）；没连上（`program_not_running`）；有没答完的调用（`driver_busy`）；
/// 重启拒了（照 `extension.restart`）；重启以后等不到握手（`program_not_running`）。
pub(crate) async fn check(
    core: &Arc<Core>,
    caller: Caller,
    params: CheckParams,
) -> Result<Value, Refusal> {
    let package = params.package;
    let drivers = core
        .system_packages()
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok())
        .map(|manifest| manifest.drivers.clone())
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    if drivers.is_empty() {
        return Err(Refusal::NO_DRIVERS);
    }
    let link = linked(core, &package)?;
    let mut checked = Vec::new();
    // 头一轮：每一份编两次。记下头一次的，重启以后比。
    let mut firsts = Vec::new();
    for decl in drivers {
        let name = format!("{package}/{}", decl.name);
        let samples = driver_check::samples(&name);
        let mut one = Checked {
            decl,
            name,
            samples: Vec::new(),
            missing: false,
        };
        let mut bodies = Vec::new();
        for sample in &samples {
            let (first, again) = (encode(&link, sample).await, encode(&link, sample).await);
            let (verdict, body) = match (first, again) {
                (Encoded::Missing, _) | (_, Encoded::Missing) => {
                    one.missing = true;
                    break;
                }
                (Encoded::Failed(why), _) | (_, Encoded::Failed(why)) => (Some(why), None),
                (Encoded::Body(first), Encoded::Body(again)) => {
                    let verdict = driver_check::first_difference(&first, &again)
                        .map(|at| format!("two encodings of the same request differ at byte {at}"));
                    (verdict, Some(first))
                }
            };
            one.samples.push((sample.name, verdict));
            bodies.push(body);
        }
        firsts.push((samples, bodies));
        checked.push(one);
    }
    // 重启扩展、等它重新握手：在跑的会话照旧在用它的不重启。
    let link = restarted(core, caller, &package, &link).await?;
    for (one, (samples, bodies)) in checked.iter_mut().zip(firsts) {
        if one.missing {
            continue;
        }
        for ((sample, body), (_, verdict)) in samples.iter().zip(bodies).zip(one.samples.iter_mut())
        {
            let (Some(body), None) = (body, verdict.as_ref()) else {
                continue;
            };
            *verdict = match encode(&link, sample).await {
                Encoded::Body(after) => driver_check::first_difference(&body, &after)
                    .map(|at| format!("the encoding changed after a restart at byte {at}")),
                Encoded::Missing => Some("driver.encode went missing after a restart".to_string()),
                Encoded::Failed(why) => Some(why),
            };
        }
        let [short, long] = driver_check::pair(&one.name);
        let verdict = match (encode(&link, &short).await, encode(&link, &long).await) {
            (Encoded::Body(short), Encoded::Body(long)) => {
                driver_check::stable_prefix(&short, &long).err()
            }
            (Encoded::Failed(why), _) | (_, Encoded::Failed(why)) => Some(why),
            _ => Some("driver.encode went missing after a restart".to_string()),
        };
        one.samples.push(("pair", verdict));
    }
    Ok(json!({
        "package": package,
        "drivers": checked.into_iter().map(report).collect::<Vec<_>>(),
    }))
}

/// 一个驱动的结果：`passed`、`failed`、`skipped`（没实现、不是 `contract` 的）。
fn report(one: Checked) -> Value {
    let cache = match one.decl.cache {
        DriverCache::Contract => "contract",
        DriverCache::BestEffort => "best_effort",
        DriverCache::None => "none",
    };
    if one.missing {
        let contract = one.decl.cache == DriverCache::Contract;
        return json!({
            "driver": one.name,
            "cache": cache,
            "result": if contract { "failed" } else { "skipped" },
            "why": "driver.encode is not implemented",
        });
    }
    let passed = one.samples.iter().all(|(_, verdict)| verdict.is_none());
    let samples: Vec<Value> = one
        .samples
        .into_iter()
        .map(|(name, verdict)| match verdict {
            None => json!({"sample": name, "ok": true}),
            Some(why) => json!({"sample": name, "ok": false, "why": why}),
        })
        .collect();
    json!({
        "driver": one.name,
        "cache": cache,
        "result": if passed { "passed" } else { "failed" },
        "samples": samples,
    })
}

/// 包 `package` 握了手的连接；有没答完的扩展驱动调用的不给（要重启）。
fn linked(core: &Core, package: &str) -> Result<Peer, Refusal> {
    let link = core.provided.linked(package).ok_or(Refusal::NOT_RUNNING)?;
    match link.streaming() {
        0 => Ok(link),
        running => Err(Refusal::driver_busy(running)),
    }
}

/// 重启扩展 `package`（`old` 是现在的连接），等它重新握手，交回新的连接。
async fn restarted(
    core: &Arc<Core>,
    caller: Caller,
    package: &str,
    old: &Peer,
) -> Result<Peer, Refusal> {
    linked(core, package)?;
    crate::extensions::restart_process(core, caller, package).await?;
    let deadline = tokio::time::Instant::now() + RELINK_WAIT;
    loop {
        if let Some(link) = core.provided.linked(package)
            && !link.same(old)
        {
            return Ok(link);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(Refusal::NOT_RUNNING);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 编一份：`driver.encode` 的回答里的 `body`（一段字，照字节比）。
async fn encode(link: &Peer, sample: &Sample) -> Encoded {
    let answered = tokio::time::timeout(
        ENCODE_WAIT,
        link.call_raw("driver.encode", sample.params.clone()),
    )
    .await;
    match answered {
        Err(_) => Encoded::Failed("driver.encode did not answer in time".to_string()),
        Ok(Err(Gone)) => Encoded::Failed("the extension went away".to_string()),
        Ok(Ok(Err(error)))
            if error.get("code").and_then(Value::as_i64) == Some(METHOD_NOT_FOUND) =>
        {
            Encoded::Missing
        }
        Ok(Ok(Err(error))) => Encoded::Failed(format!(
            "driver.encode returned an error: {}",
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("no message")
        )),
        Ok(Ok(Ok(result))) => match result.get("body").and_then(Value::as_str) {
            Some(body) => Encoded::Body(body.as_bytes().to_vec()),
            None => Encoded::Failed("the answer to driver.encode has no body".to_string()),
        },
    }
}

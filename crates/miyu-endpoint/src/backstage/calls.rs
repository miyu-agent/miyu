//! `package.methods`、`package.call`、`method.call`（施工 F-6 中，`package-pages.md`「`package.methods`、`package.call`、
//! `method.call`」）：扩展登记它的后台页要调的方法，头调，核心反向转给扩展、交回它回的。登记不缓存：连接断了，调到的回
//! `program_not_running`；没在跑、还没登记的也是，不替它拉起（施工 BUS-2 照实改了这一句，原来写的「用到时拉起、最多等
//! 60 秒」代码里一直没有）。转发那一段（[`forward`]）总线的 `bus.call` 共用（施工 BUS-2，`bus.rs`）。

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::Core;
use crate::extensions::Instance;
use crate::hello::Caller;
use crate::refusal::Refusal;
use crate::reverse::{Gone, Peer};

/// 不写时限的等多久。
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// 时限能写的范围，毫秒（同工具的）。
const TIMEOUT_RANGE: std::ops::RangeInclusive<u64> = 1000..=600_000;

/// 哪个包登记了哪些方法、由哪个连接答；包推给人的广播（施工 IM-1 中下二，`events.rs`）。
#[derive(Default)]
pub(crate) struct Methods {
    table: Mutex<HashMap<Instance, Registered>>,
    pub(crate) events: super::events::Events,
    /// 核心按需起的后台页服务（施工 BP-3）。
    pub(crate) host: super::host::Host,
}

/// 一个包登记的。
struct Registered {
    peer: Peer,
    methods: BTreeMap<String, Duration>,
}

/// `package.methods` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterParams {
    methods: Vec<MethodParams>,
}

/// 登记的一个方法。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MethodParams {
    name: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

/// `package.call` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CallParams {
    package: String,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

/// `package.methods`：换掉这个包上一次登记的，交回 `{"methods": 个数}`。不是核心拉起的扩展的连接 `not_an_extension`；
/// 名字写法不对、重复、时限不在范围里的 `bad_params`，什么都不换。
pub(crate) fn register(
    core: &Core,
    caller: &Caller,
    params: RegisterParams,
) -> Result<Value, Refusal> {
    let Some(package) = caller.package.as_deref() else {
        return Err(Refusal::NOT_AN_EXTENSION);
    };
    let mut methods = BTreeMap::new();
    for method in params.methods {
        let timeout = method.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        if !valid(&method.name)
            || methods.contains_key(&method.name)
            || !TIMEOUT_RANGE.contains(&timeout)
        {
            return Err(Refusal::BAD_PARAMS);
        }
        methods.insert(method.name, Duration::from_millis(timeout));
    }
    let count = methods.len();
    let instance = Instance::of(package);
    let label = instance.label();
    core.backstage
        .table
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            instance,
            Registered {
                peer: caller.reverse.clone(),
                methods,
            },
        );
    tracing::info!(target: "miyu::endpoint", package = label, methods = count, "page methods registered");
    // 总线上可不可用跟着变（施工 BUS-2）：订着 `bus`、`packages` 的照这一刻推。
    core.package_changes.notify(package);
    Ok(json!({ "methods": count }))
}

/// `package.call`：照登记的转给扩展，回什么交回什么。`from` 是调的扩展的包（施工 X-18）：它要声明了 `packages.call`（连接那一头照
/// `connection/rights.rs` 的表查，施工 X-30），只调得到对方清单 `[export]` 里的方法，转过去时带上 `from`；人的连接没有 `from`，后台页登记的都调得到。回应里对方拿得到的 blob，
/// `from` 也拿得到（施工 W-6 三补，[`crate::attach::Grants::pass`]）。转给 `caller` 替的那个人那一份（施工 IM-1 中下）。
pub(crate) async fn call(
    core: &Core,
    caller: &Caller,
    params: CallParams,
) -> Result<Value, Refusal> {
    let from = caller.package.as_deref();
    let packages = core.system_packages();
    let manifest = |id: &str| {
        packages
            .iter()
            .find(|found| found.id == id)
            .and_then(|found| found.read.as_ref().ok())
    };
    let Some(target) = manifest(&params.package) else {
        return Err(Refusal::UNKNOWN_PACKAGE);
    };
    // 扩展调的：声明了 `packages.call` 没有，连接那一头照表查过了（施工 X-30，`connection/rights.rs`）；这里只看对方给没给出来。
    if from.is_some() && !target.export.exports(&params.method) {
        return Err(Refusal::not_exported(&params.method));
    }
    forward(core, from, &params.package, &params.method, params.params).await
}

/// 转给包 `package` 登记的方法 `method`，`from` 是调的扩展（人的连接没有）：查过能不能调的由调的一方做。参数里的 blob 先交给
/// 被调的一方（施工 BUS-2：扩展只交得出它自己拿得到的，人的连接都交），回应里对方拿得到的再交给 `from`。扩展回的错带的码
/// 照 `error.data.code`（字的，JSON-RPC 的 `code` 只能是整数，施工 BUS-2）、没有的照 `error.code` 放进 `data.code`。
pub(crate) async fn forward(
    core: &Core,
    from: Option<&str>,
    package: &str,
    method: &str,
    params: Option<Value>,
) -> Result<Value, Refusal> {
    let params = CallParams {
        package: package.to_string(),
        method: method.to_string(),
        params,
    };
    let instance = Instance::of(&params.package);
    let peer = core
        .backstage
        .peer_of(&instance)
        .ok_or(Refusal::NOT_RUNNING)?;
    let timeout = core
        .backstage
        .table
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&instance)
        .and_then(|registered| registered.methods.get(&params.method).copied())
        .ok_or_else(|| Refusal::unregistered(&params.method))?;
    let given = params.params.unwrap_or_else(|| json!({}));
    match from {
        Some(from) => core.blob_grants.pass(from, &params.package, &given),
        None => core.blob_grants.hand(&params.package, &given),
    }
    let mut request = json!({"method": params.method, "params": given});
    if let Some(from) = from {
        request["from"] = Value::String(from.to_string());
    }
    match tokio::time::timeout(timeout, peer.call("method.call", request)).await {
        Err(_) => Err(Refusal::METHOD_TIMEOUT),
        Ok(Err(Gone)) => Err(Refusal::NOT_RUNNING),
        Ok(Ok(Ok(result))) => {
            if let Some(from) = from {
                core.blob_grants.pass(&params.package, from, &result);
            }
            Ok(result)
        }
        Ok(Ok(Err(error))) => Err(Refusal::method_failed(
            error
                .get("message")
                .and_then(Value::as_str)
                .map_or_else(|| error.to_string(), str::to_string),
            error
                .pointer("/data/code")
                .filter(|code| code.is_string())
                .or_else(|| error.get("code"))
                .cloned(),
        )),
    }
}

impl Methods {
    /// 包 `package` 这时登记着、还连着的方法名（施工 BUS-2：`ready` 照它算）；没登记、断了的没有。
    pub(crate) fn registered(&self, package: &str) -> Option<Vec<String>> {
        let instance = Instance::of(package);
        self.peer_of(&instance)?;
        let table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        Some(table.get(&instance)?.methods.keys().cloned().collect())
    }

    /// `instance` 登记了方法、还连着的连接。
    fn peer_of(&self, instance: &Instance) -> Option<Peer> {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(instance)
            .map(|registered| registered.peer.clone())
            .filter(|peer| !peer.closed())
    }
}

/// 方法名：小写字母开头，小写字母、数字、`_`、`.`、`-`，1 到 64 个。
pub(crate) fn valid(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
}

#[cfg(test)]
mod tests;

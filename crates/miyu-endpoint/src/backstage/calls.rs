//! `package.methods`、`package.call`、`method.call`（施工 F-6 中，`package-pages.md`「`package.methods`、`package.call`、
//! `method.call`」）：扩展登记它的后台页要调的方法，头调，核心反向转给扩展、交回它回的。登记不缓存：连接断了，调到的回
//! `not_running`。
//!
//! 施工 IM-1 中下：照（包，账号）记、照（包，账号）转。每人一份的包，谁调就转给谁那一份（页面是看页面的那个人，扩展调的是它替
//! 的那个人）；没在跑的照用到时拉起，等它登记了方法再转，最多等 60 秒。

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::ContentHash;

use crate::Core;
use crate::extensions::Instance;
use crate::hello::Caller;
use crate::refusal::Refusal;
use crate::reverse::{Gone, Peer};

/// 不写时限的等多久。
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// 时限能写的范围，毫秒（同工具的）。
const TIMEOUT_RANGE: std::ops::RangeInclusive<u64> = 1000..=600_000;

/// 哪一份（包，账号）登记了哪些方法、由哪个连接答。
#[derive(Default)]
pub(crate) struct Methods {
    table: Mutex<HashMap<Instance, Registered>>,
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
    let instance = Instance::serving(core, package, caller.acting_for());
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
    // 等着这一份登记方法的（用到时拉起的）醒过来。
    core.provided.nudge();
    tracing::info!(target: "miyu::endpoint", package = label, methods = count, "page methods registered");
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
    super::opened_to(core, caller, &params.package)?;
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
    if from.is_some() && !target.export.contains(&params.method) {
        return Err(Refusal::not_exported(&params.method));
    }
    let instance = Instance::serving(core, &params.package, caller.acting_for());
    let found = |instance: &Instance| core.backstage.peer_of(instance);
    let peer = match (found(&instance), &instance.account) {
        (Some(peer), _) => peer,
        (None, Some(_)) => core
            .provided
            .wait(&instance, found)
            .await
            .ok_or(Refusal::NOT_RUNNING)?,
        (None, None) => return Err(Refusal::NOT_RUNNING),
    };
    let timeout = core
        .backstage
        .table
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&instance)
        .and_then(|registered| registered.methods.get(&params.method).copied())
        .ok_or_else(|| Refusal::unregistered(&params.method))?;
    let mut request =
        json!({"method": params.method, "params": params.params.unwrap_or_else(|| json!({}))});
    if let Some(from) = from {
        request["from"] = Value::String(from.to_string());
    }
    match tokio::time::timeout(timeout, peer.call("method.call", request)).await {
        Err(_) => Err(Refusal::METHOD_TIMEOUT),
        Ok(Err(Gone)) => Err(Refusal::NOT_RUNNING),
        Ok(Ok(Ok(result))) => {
            if let Some(from) = from {
                let passed = core.blob_grants.pass(&params.package, from, &result);
                let answered = instance
                    .account
                    .clone()
                    .unwrap_or_else(|| core.account_of(Some(&params.package)));
                hand_over(core, (answered, caller.account.clone()), passed).await;
            }
            Ok(result)
        }
        Ok(Ok(Err(error))) => Err(Refusal::method_failed(
            error
                .get("message")
                .and_then(Value::as_str)
                .map_or_else(|| error.to_string(), str::to_string),
            error.get("code").cloned(),
        )),
    }
}

/// 答的那一份的账号 `from` 转给调的那一份的账号 `to` 的 blob（施工 Y-5 中上）：两个包各是自己的账号、各有自己的附件库，从答的
/// 那边拷进调的那边，调的包才读得到。每人一份的那一份的账号是那个人（施工 IM-1 中下）；同一个账号的不用拷。拷不成的记一行，
/// 许可照样给：读的时候照 `unknown_blob` 答。
async fn hand_over(
    core: &Core,
    (from, to): (miyu_kernel::id::AccountId, miyu_kernel::id::AccountId),
    passed: Vec<ContentHash>,
) {
    if passed.is_empty() || from == to {
        return;
    }
    let root = core.root.clone();
    let copied =
        tokio::task::spawn_blocking(move || crate::attach::copy_over(&root, &from, &to, &passed))
            .await;
    if !matches!(copied, Ok(Ok(()))) {
        tracing::warn!(target: "miyu::endpoint", "passed blobs not copied");
    }
}

impl Methods {
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
pub(super) fn valid(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
}

#[cfg(test)]
mod tests;

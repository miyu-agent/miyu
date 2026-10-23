//! 在后台答的请求（施工 8-20 补、R-7 补、V-6 上）：哪些在后台答、怎么办。

use std::sync::Arc;

use serde_json::Value;

use super::params;
use crate::hello::{Caller, Peer};
use crate::refusal::Refusal;
use crate::wire::Request;
use crate::{Core, backstage, commands, files, job_output, memory, models};

/// 在后台答的自带请求（施工 8-20 补，`protocol.md`「一个连接」第 1 条）：`model.call` 要等模型说完，几秒到几分钟；现在就整理
/// 记忆（`memory.dream`，`command.run` 的 `/dream`，施工 R-7 补）几秒到十几秒。同一个连接后面的请求不等它，回应照 `id`
/// 对上。`command.run` 的照命令表问（[`commands::answered_later`]），别的命令照旧按顺序答。
pub(crate) fn answered_later(core: &Core, request: &Request) -> bool {
    match request.method.as_str() {
        "model.call" | "memory.dream" | "package.call" => true,
        "command.run" => commands::answered_later(core, request),
        // 要等的在后台答（施工 V-6 上）：清单建完、输出长了才回，不挡这个连接别的请求。
        "fs.find" | "job.output" => {
            request.params.get("wait").and_then(Value::as_bool) == Some(true)
        }
        _ => false,
    }
}

/// 在后台办一条 [`answered_later`] 认的请求，连接的这一头是 `peer`（`command.run` 的回执照它的语言写），连接是谁是 `caller`：
/// 它的包是核心拉起的扩展的（施工 X-18：`package.call` 照它查能调哪几个，人的连接没有），`model.call` 的图照它的账号找
/// （施工 Y-2中下）。
pub(crate) async fn call_later(
    core: &Arc<Core>,
    peer: Peer,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "model.call" => models::call(core, caller.acting_for(), params(request)?).await,
        "memory.dream" => memory::dream::call(core, caller.acting_for(), &request.params).await,
        "package.call" => backstage::call(core, caller.package.as_deref(), params(request)?).await,
        "command.run" => commands::run(core, &peer, &request.id, params(request)?).await,
        "fs.find" => files::find(core, params(request)?).await,
        "job.output" => job_output::read(core, params(request)?).await,
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

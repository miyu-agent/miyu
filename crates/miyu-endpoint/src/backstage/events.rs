//! 包推给人（施工 IM-1 中下二，im 设计 07 第 4.6 节第 3 样）：扩展调 `package.emit {"event", "data"?}`，核心推
//! `package.event {"package", "event", "data"}` 给它替的那个账号、订了这个包（`subscribe {"stream": "package", "package"}`）的
//! 连接。页面照它更新，不用轮询。不存盘、不补：没订着的收不到，重连也不补；掉了队推 `resync`（带 `package`）。
//!
//! 一条是「账号、换行、包、换行、推送的那一行」，订阅的转发任务只挑自己账号、自己订的包的（`subscriptions/listed.rs`）。

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use miyu_kernel::id::AccountId;

use crate::hello::Caller;
use crate::refusal::Refusal;

/// 广播里最多攒几条没收的。
const PUSHES: usize = 256;

/// `data` 写成 JSON 最多多大：页面要的是变了什么，不是整份数据。
const DATA: usize = 256 * 1024;

/// 包推给人的广播。
#[derive(Debug)]
pub(crate) struct Events(broadcast::Sender<String>);

impl Default for Events {
    fn default() -> Events {
        Events(broadcast::channel(PUSHES).0)
    }
}

impl Events {
    /// 收推送的一头。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<String> {
        self.0.subscribe()
    }
}

/// `package.emit` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmitParams {
    event: String,
    #[serde(default)]
    data: Option<Value>,
}

/// `package.emit`：推给 `caller` 替的那个账号、订了它这个包的连接，回 `{}`。不是核心拉起的扩展的连接 `not_an_extension`；
/// 事件名写法同后台页的方法名，不对的、`data` 太大的 `bad_params`。
pub(crate) fn emit(
    core: &crate::Core,
    caller: &Caller,
    params: EmitParams,
) -> Result<Value, Refusal> {
    let Some(package) = caller.package.as_deref() else {
        return Err(Refusal::NOT_AN_EXTENSION);
    };
    let data = params.data.unwrap_or(Value::Null);
    if !super::calls::valid(&params.event) || data.to_string().len() > DATA {
        return Err(Refusal::BAD_PARAMS);
    }
    let line = json!({"jsonrpc": "2.0", "method": "package.event",
        "params": {"package": package, "event": params.event, "data": data}});
    let account = &core.admin;
    if core
        .backstage
        .events
        .0
        .send(format!("{}\n{package}\n{line}", account.as_str()))
        .is_err()
    {
        // 没有订着的连接：不用说。
    }
    Ok(json!({}))
}

/// 推送 `pushed` 是不是 `account` 的、包 `package` 的：是的交回推送的那一行。
pub(crate) fn mine<'a>(pushed: &'a str, account: &AccountId, package: &str) -> Option<&'a str> {
    let (whose, rest) = pushed.split_once('\n')?;
    let (which, line) = rest.split_once('\n')?;
    (whose == account.as_str() && which == package).then_some(line)
}

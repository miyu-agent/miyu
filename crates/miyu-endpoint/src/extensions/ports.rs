//! 端口登记（施工 IM-5，接入通讯平台设计 06 第 4.6 节第 5 样）：扩展先登记要听的端口，撞了的当场知道，不用等系统说绑不上。
//!
//! - 只是一张表，不是核心替它去听：真正抢端口的是操作系统。
//! - `port.claim {"port"}`：没人登记的、登记它的连接已经断了的记成这个连接的，回 `{}`；同一个连接再登记照回 `{}`；别人登记着的
//!   回 `port_taken`，不说是谁。
//! - `port.release {"port"}`：是这个连接登记的拿掉，别的不动，都回 `{}`。
//! - 进程退出、连接断了（包括答应了 `process.retire` 的）照这个连接登记的都算放开：表里记着连接，查的时候断了的当没有，不用另做
//!   收尾。核心重启表就空了，扩展起来再登记。
//! - `port.list`：只给管理员（别人 `not_permitted`）看整张表 `{"ports": [{"port", "package", "account"?}]}`，照端口排；`account` 是每人一份的那个人。

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use serde::Deserialize;
use serde_json::{Value, json};

use super::Instance;
use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;
use crate::reverse::Peer;

/// 一个端口登记在谁那儿。
struct Holder {
    instance: Instance,
    peer: Peer,
}

/// 端口表：核心一份，只在内存里。
#[derive(Default)]
pub(crate) struct Ports(Mutex<BTreeMap<u16, Holder>>);

impl Ports {
    fn table(&self) -> std::sync::MutexGuard<'_, BTreeMap<u16, Holder>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 把 `port` 记成 `instance` 那个连接 `peer` 的：别的还连着的连接登记着的交回 `false`，不动。
    fn claim(&self, port: u16, instance: &Instance, peer: &Peer) -> bool {
        let mut table = self.table();
        if let Some(held) = table.get(&port)
            && !held.peer.closed()
            && !held.peer.same(peer)
        {
            return false;
        }
        table.insert(
            port,
            Holder {
                instance: instance.clone(),
                peer: peer.clone(),
            },
        );
        true
    }

    /// `peer` 登记的 `port` 拿掉；别的不动。
    fn release(&self, port: u16, peer: &Peer) {
        let mut table = self.table();
        if table.get(&port).is_some_and(|held| held.peer.same(peer)) {
            table.remove(&port);
        }
    }

    /// 还连着的登记，照端口排；断了的顺手拿掉。
    fn held(&self) -> Vec<(u16, Instance)> {
        let mut table = self.table();
        table.retain(|_, held| !held.peer.closed());
        table
            .iter()
            .map(|(port, held)| (*port, held.instance.clone()))
            .collect()
    }
}

/// `port.claim`、`port.release` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortParams {
    port: u16,
}

/// `port.claim`：见模块说明。端口是 0 的 `bad_params`，不是核心拉起的扩展的连接 `not_an_extension`。
pub(crate) fn claim(core: &Core, caller: &Caller, params: PortParams) -> Result<Value, Refusal> {
    let instance = instance(core, caller, params.port)?;
    match core
        .extensions
        .ports
        .claim(params.port, &instance, &caller.reverse)
    {
        true => Ok(json!({})),
        false => Err(Refusal::PORT_TAKEN),
    }
}

/// `port.release`：见模块说明。
pub(crate) fn release(core: &Core, caller: &Caller, params: PortParams) -> Result<Value, Refusal> {
    instance(core, caller, params.port)?;
    core.extensions.ports.release(params.port, &caller.reverse);
    Ok(json!({}))
}

/// `port.list`：只给管理员，别人 `not_permitted`。
pub(crate) fn list(core: &Core, caller: &Caller) -> Result<Value, Refusal> {
    if !crate::login::admins::is_admin(core, caller.acting_for()) {
        return Err(Refusal::NOT_PERMITTED);
    }
    let ports: Vec<Value> = core
        .extensions
        .ports
        .held()
        .into_iter()
        .map(|(port, instance)| {
            let mut one = json!({"port": port, "package": instance.package});
            if let Some(account) = instance.account {
                one["account"] = json!(account.as_str());
            }
            one
        })
        .collect();
    Ok(json!({"ports": ports}))
}

/// 连接是哪一份：不是核心拉起的扩展的 `not_an_extension`；端口是 0 的 `bad_params`。
fn instance(core: &Core, caller: &Caller, port: u16) -> Result<Instance, Refusal> {
    let package = caller.package.as_deref().ok_or(Refusal::NOT_AN_EXTENSION)?;
    if port == 0 {
        return Err(Refusal::bad_field("port"));
    }
    Ok(Instance::serving(core, package, caller.acting_for()))
}

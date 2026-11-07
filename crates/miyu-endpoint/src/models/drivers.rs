//! 扩展提供的模型驱动的通道，核心这一头（施工 MD-2，设计 66 第五、八、十节）：实现执行器的 [`DriverPort`]，核心起来时装进模型资料
//! （[`Core::start_drivers`]）。
//!
//! - 在不在（[`DriverPort::present`]）：装着、清单里有这个驱动名、批了 `model.driver`、开着、没停下的才在，交它的声明；不在的交一句
//!   原因，执行器当这家供应商用不了、不重试（第八节第 5 条，第十节第 4 条：关掉、卸掉、撤了批准的都是这样）。
//! - 发（[`DriverPort::call`]）：找这个包握了手的连接；没有的（退避中、刚拉起）当可重试，退避中的带上还要等多久。登记这一次的
//!   增量、发 `driver.call`；不等了（执行器丢掉）的发 `driver.cancel`，增量不再收。参数里有 key，不进日志。

use std::sync::{Arc, Weak};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use miyu_config::package::Capability;
use miyu_models::provider::DriverRef;
use miyu_session::{Broken, Declared, DriverPending, DriverPort, NotYet};

use crate::Core;
use crate::extensions::State;
use crate::reverse::{Gone, Peer};

/// 核心的那一头：核心没了的当驱动不在。
#[derive(Debug)]
struct Drivers(Weak<Core>);

impl Core {
    /// 核心起来时调一次（施工 MD-2）：模型资料里装上扩展驱动的通道。
    pub fn start_drivers(self: &Arc<Self>) {
        self.model_data
            .on_drivers(Arc::new(Drivers(Arc::downgrade(self))));
    }
}

impl DriverPort for Drivers {
    fn present(&self, driver: &DriverRef) -> Result<Declared, String> {
        let core = self.0.upgrade().ok_or("the core is shutting down")?;
        let packages = core.system_packages();
        let manifest = packages
            .iter()
            .find(|found| found.id == driver.package)
            .and_then(|found| found.read.as_ref().ok())
            .ok_or("its package is not installed")?;
        let declared = manifest
            .drivers
            .iter()
            .find(|declared| declared.name == driver.name)
            .ok_or("its package declares no driver by that name")?;
        if !crate::extensions::held(&core, &driver.package).contains(&Capability::ModelDriver) {
            return Err("its package is not approved to provide model drivers".to_string());
        }
        match core.extensions.status(&driver.package).state {
            State::Off => Err("its package is turned off".to_string()),
            State::Stopped(reason) => Err(format!("its extension stopped: {}", reason.as_str())),
            State::Starting { .. } | State::Running { .. } | State::Waiting { .. } => {
                Ok(Declared {
                    key: declared.key,
                    images: declared.images,
                })
            }
        }
    }

    fn call(
        &self,
        driver: &DriverRef,
        call: &str,
        params: String,
        deltas: mpsc::UnboundedSender<Value>,
    ) -> Result<DriverPending, NotYet> {
        let peer = self.link(driver)?;
        peer.stream(call, deltas);
        let live = Live {
            peer,
            call: call.to_string(),
            done: false,
        };
        Ok(Box::pin(async move {
            // 整个搬进来：丢下这个 future 时它跟着丢下，才发得了 `driver.cancel`。
            let mut live = live;
            let answered = live.peer.call_raw("driver.call", params).await;
            live.done = true;
            broken(answered)
        }))
    }

    fn ask(
        &self,
        driver: &DriverRef,
        method: &str,
        params: String,
    ) -> Result<DriverPending, NotYet> {
        let peer = self.link(driver)?;
        let method = method.to_string();
        Ok(Box::pin(async move {
            broken(peer.call_raw(&method, params).await)
        }))
    }
}

impl Drivers {
    /// 驱动 `driver` 所在的扩展握了手的连接；没有的（退避中、刚拉起）交 [`NotYet`]：退避中的照它还要等多久，刚拉起的不说。
    fn link(&self, driver: &DriverRef) -> Result<Peer, NotYet> {
        let core = self.0.upgrade().ok_or(NotYet { retry_in_ms: None })?;
        core.provided.linked(&driver.package).ok_or_else(|| {
            let retry_in_ms = match core.extensions.status(&driver.package).state {
                State::Waiting { until } => Some(millis(
                    until.saturating_duration_since(tokio::time::Instant::now()),
                )),
                _ => None,
            };
            NotYet { retry_in_ms }
        })
    }
}

/// 等到的回应照执行器认的分：回答原样、JSON-RPC 的错误交原话、连接断了。
fn broken(answered: Result<Result<Value, Value>, Gone>) -> Result<Value, Broken> {
    match answered {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(error)) => Err(Broken::Refused(
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("no message")
                .to_string(),
        )),
        Err(Gone) => Err(Broken::Gone),
    }
}

/// 在路上的一次：说完、不等了都不再收它的增量；没等到回答就被丢下的（执行器叫停、超时、增量不对）发 `driver.cancel`。
struct Live {
    peer: Peer,
    call: String,
    done: bool,
}

impl Drop for Live {
    fn drop(&mut self) {
        self.peer.unstream(&self.call);
        if !self.done
            && !self
                .peer
                .notify("driver.cancel", json!({"call": self.call}))
        {
            // 写队列满了、连接断了的发不出去：它回答时核心照样丢掉对不上的回答，增量也不再收。
        }
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;

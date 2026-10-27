//! 找每人一份的那一份（施工 IM-1 中，接入通讯平台设计 06 第 4.6 节第 1、6 样）：她替会话的属主调每人一份的包的工具，转给他那
//! 一份；还没起、退出了的，核心当场替他拉起（他能用这个包才起，`Core::start_for`），等它连上来、登记了工具再转。最多等
//! [`WAIT`]；起不来、停下了、没等到的交回没有，工具照「暂时不可用」答。

use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;

use miyu_kernel::id::AccountId;

use super::Provided;
use crate::extensions::Instance;
use crate::reverse::Peer;

/// 最多等多久：同扩展命令。
const WAIT: Duration = Duration::from_secs(60);

impl Provided {
    /// 包 `package` 替 `owner` 的那一份的连接：在的直接交回，没有的拉起、等它连上、登记了工具。
    pub(super) async fn reach(&self, package: &str, owner: &AccountId) -> Option<Peer> {
        let instance = Instance {
            package: package.to_string(),
            account: Some(owner.clone()),
        };
        self.wait(&instance, Self::live).await
    }

    /// 包 `package` 替 `account` 的那一份的连接（施工 J-4：到点推给建它的扩展），只要握了手、不用登记工具：每人一份的没在跑
    /// 的拉起、等它握手；只有一份的是连着的那一份，没连着的没有（它归开机、扩展管理拉起）。
    pub(crate) async fn toward(
        &self,
        core: &crate::Core,
        package: &str,
        account: &AccountId,
    ) -> Option<Peer> {
        if super::per_account(core, package) {
            let instance = Instance {
                package: package.to_string(),
                account: Some(account.clone()),
            };
            self.wait(&instance, Self::linked_of).await
        } else {
            self.linked_of(&Instance::system(package))
        }
    }

    /// `instance` 照 `found` 找得到的连接：找得到的直接交回，没有的替那个人拉起，等到找得到。最多等 [`WAIT`]；起不来、停下了、
    /// 没等到的交回没有。
    async fn wait(
        &self,
        instance: &Instance,
        found: fn(&Self, &Instance) -> Option<Peer>,
    ) -> Option<Peer> {
        if let Some(peer) = found(self, instance) {
            return Some(peer);
        }
        let weak = self.core.get()?.clone();
        let core = weak.upgrade()?;
        let mut arrived = self.arrived.subscribe();
        let mut changed = core.extensions.subscribe();
        if !core.start_for(instance) {
            return None;
        }
        drop(core);
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            if let Some(peer) = found(self, instance) {
                return Some(peer);
            }
            // 停下了、被关了的不等（起不来的退避中照样等，它还会再起）。
            let core = weak.upgrade()?;
            if !core.extensions.status_of(instance).state.busy() {
                return None;
            }
            drop(core);
            tokio::select! {
                arrival = arrived.changed() => arrival.ok()?,
                change = changed.recv() => {
                    if matches!(change, Err(RecvError::Closed)) {
                        return None;
                    }
                }
                () = tokio::time::sleep_until(deadline) => {
                    tracing::warn!(target: "miyu::endpoint", package = instance.label(), "extension not reached in time");
                    return None;
                }
            }
        }
    }

    /// `instance` 连着的连接：断了的不算。
    fn live(&self, instance: &Instance) -> Option<Peer> {
        self.peer_of(instance).filter(|peer| !peer.closed())
    }
}

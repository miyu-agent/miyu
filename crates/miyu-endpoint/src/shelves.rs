//! 每人一个工具目录架子（施工 U23 中中，设计 06 U23）：机主这一侧（[`Core::machine_side`]：机主的、包的账号的）的会话拿核心那一个
//! （`Core::tools`）；别人的各一个叠在它上面的（`miyu_tool::Shelf::over`），用到时造、一直留着：拿掉机主私人的 `~home`（他家目录
//! 里放的脚本、个人设置里的 MCP 服务）。核心那一个换代了（装卸包、扩展登记、脚本改了），叠在上面的读的时候跟着重拼。执行器、
//! 权限策略、放行出沙盒认来处、回合开头换快照都照会话手里这一个，一处定、处处对。
//!
//! 成员自己的脚本（他家目录的、他那一层包带的）进他这一个随 U23 中下。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::id::AccountId;
use miyu_store::scripts::HOME_PACKAGE;
use miyu_tool::{Catalog, Shelf};

use crate::Core;

/// 造好的那几个：账号到他的架子。
#[derive(Debug, Default)]
pub(crate) struct Shelves(Mutex<BTreeMap<AccountId, Shelf>>);

impl Core {
    /// 属主是 `account` 的会话拿的工具目录架子：机主这一侧的是核心那一个，别人的是叠在上面、拿掉 `~home` 的（造过的照旧那一个）。
    pub(crate) fn shelf_of(&self, account: &AccountId) -> Shelf {
        if self.machine_side(account) {
            return self.tools.clone();
        }
        self.shelves
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(account.clone())
            .or_insert_with(|| {
                Shelf::over(
                    &self.tools,
                    Arc::new(|catalog: &Catalog| catalog.without(HOME_PACKAGE)),
                )
            })
            .clone()
    }
}

#[cfg(test)]
mod tests;

//! 每人一份装着的包（施工 U23 上下，设计 06 U23）：系统那一份（出厂的、系统那一层，[`Core::system_packages`]）是机器上的，
//! 进程、包账号、扩展照它；一个人看得到、用得到的（他的会话的工具面、技能、预设，`package.list`）是系统那一份接上他自己装的那一层
//! （`home/<账号>/packages/`，[`Core::packages_of`]）。
//!
//! 他那一层读在最后：同编号的盖过系统那一份（施工 U23 下上，系统那一份带程序的不盖），子命令名、功能编号和系统那一份撞了的报在
//! 他那份上（先到先得）。用到时造一份留着，连同造它时的系统那一份：取的时候系统那一份换了（装卸，[`Core::reload_packages`]）的
//! 当场重造，正读着的时候换了也不会留下旧的；装卸以后另外整个清掉，省得不再用的人那几份一直占着；他自己那一层装卸了的清掉他那
//! 一份（[`Own::forget_account`]）。他那一层是空的就是系统那一份本身。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::id::AccountId;
use miyu_store::packages::{Found, Layer, Packages};

use super::{compiled, settle};
use crate::Core;

/// 造好的那几份：账号到造它时的系统那一份、他看得到的包。
#[derive(Debug, Default)]
pub(crate) struct Own(Mutex<BTreeMap<AccountId, Made>>);

/// 造好的一份。
#[derive(Debug)]
struct Made {
    /// 造它时的系统那一份：和这时的不是同一份的作废。
    system: Arc<Vec<Found>>,
    /// 造它时他关了的包（施工 U23 末上）：和这时的不一样的作废。
    off: BTreeSet<String>,
    /// 他看得到的包。
    found: Arc<Vec<Found>>,
}

impl Own {
    /// 清掉（系统那一层变了）。
    pub(crate) fn forget(&self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// 清掉账号 `account` 那一份（施工 U23 下上：他自己那一层装卸了）。
    pub(crate) fn forget_account(&self, account: &AccountId) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(account);
    }
}

impl Core {
    /// 系统那一份软件包清单（施工 F-5 上：装卸以后当场换；施工 U23 上下改名）：出厂的、系统那一层。机器上的事（进程、包账号、
    /// 扩展、MCP 服务、装卸）照它；替某个人看、用的照 [`Core::packages_of`]。
    pub fn system_packages(&self) -> Arc<Vec<Found>> {
        Arc::clone(&self.packages.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// 账号 `account` 看得到的软件包（施工 U23 上下）：系统那一份接上他自己装的那一层，去掉他关了的（施工 U23 末上，
    /// `Core::packages_off`）。包的账号（平台桥、替机主干活的扩展）照系统那一份。
    pub fn packages_of(&self, account: &AccountId) -> Arc<Vec<Found>> {
        let system = self.system_packages();
        if self.package_accounts().contains(account) {
            return system;
        }
        let off = self.packages_off(account);
        if let Some(made) = self
            .own_packages
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(account)
            .filter(|made| Arc::ptr_eq(&made.system, &system) && made.off == off)
        {
            return Arc::clone(&made.found);
        }
        let found = self.read_own(account, &system, &off);
        self.own_packages
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                account.clone(),
                Made {
                    system,
                    off,
                    found: Arc::clone(&found),
                },
            );
        found
    }

    /// 账号 `account` 看得到的软件包，他关了的也在（施工 U23 下上：列表、条目、装卸照它比）：同 [`Core::packages_of`]，不去掉
    /// 他关了的、不留着。碰磁盘。
    pub(crate) fn packages_seen(&self, account: &AccountId) -> Arc<Vec<Found>> {
        let system = self.system_packages();
        if self.package_accounts().contains(account) {
            return system;
        }
        self.read_own(account, &system, &BTreeSet::new())
    }

    /// 读一份：系统那一份 `system` 接上 `account` 那一层（[`Packages::read_over`]），他那一层再照核心这边认一遍（没编进来的内置包、
    /// 配置项和核心模块撞名），去掉他关了的 `off`。他那一层什么都没有、什么都没关的交回系统那一份。碰磁盘。
    fn read_own(
        &self,
        account: &AccountId,
        system: &Arc<Vec<Found>>,
        off: &BTreeSet<String>,
    ) -> Arc<Vec<Found>> {
        let places = Packages::of_account(&self.resources, &self.root, &self.admin, account);
        let (mut own, mut found): (Vec<Found>, Vec<Found>) = places
            .read_over(system)
            .into_iter()
            .partition(|one| one.layer == Layer::User);
        found.retain(|one| !off.contains(&one.id));
        own.retain(|one| !off.contains(&one.id));
        if own.is_empty() && off.is_empty() {
            return Arc::clone(system);
        }
        if let Some(built_in) = &self.built_in {
            compiled(&mut own, built_in);
        }
        let items = self.config().items().to_vec();
        // 撞了核心模块名的问题 `settle` 写进他那份（`Err`），随清单交出去；交回的是拼出来的配置项，这一步不进他的配置（成员的配置
        // 照系统那一层造，他那一层的配置项随 U23 下放开成员装再接）。
        let _ = settle(&mut own, &items);
        found.extend(own);
        found.sort_by(|one, other| (&one.id, one.layer).cmp(&(&other.id, other.layer)));
        Arc::new(found)
    }
}

#[cfg(test)]
mod tests;

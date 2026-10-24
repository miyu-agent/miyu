//! 系统账号（施工 O-4 下，`docs/construction/O-4-系统账号（下）.md`，`06-多用户与身份.md` U14）：声明了系统账号的包各一个，
//! 账号名就是包的编号。照核心手里这时的清单算（装卸当场换，施工 F-5 下），不落盘：`home/<编号>/` 就是它在磁盘上的样子。
//! 核心拉起的这个包的扩展以它的身份连进来；它的东西管理员能看能管，它没有自己的记忆（归机主，[`Core::memory_owner`]）。
//!
//! 施工 Y-5 中上（设计 06 U16）：每个带进程的包都是自己的账号（[`Core::package_accounts`]），起来时一样建家目录；声明了系统账号
//! 的（平台桥）替平台上的外人干活，别的替机主干活（[`Core::serving`]）。记忆人人各一份（施工 Y-7）：
//! 别的包开的会话归机主。施工 Y-2 下下中下起用量人人都记（[`Core::usage_accounts`]）。

use std::sync::Arc;

use miyu_kernel::id::AccountId;

use crate::Core;

/// 运行日志的目标。
const TARGET: &str = "miyu::endpoint";

impl Core {
    /// 这次起来认的系统账号，照编号排。
    pub(crate) fn system_accounts(&self) -> Vec<AccountId> {
        miyu_store::packages::system_accounts(&self.system_packages())
    }

    /// `account` 是不是这次起来认的系统账号。
    pub(crate) fn is_system(&self, account: &AccountId) -> bool {
        self.system_accounts().contains(account)
    }

    /// 管理员和系统账号：场所会话只会在它们名下。
    pub(crate) fn accounts(&self) -> Vec<AccountId> {
        std::iter::once(self.admin.clone())
            .chain(self.system_accounts())
            .collect()
    }

    /// 用量照哪几个账号补（施工 Y-2 下下中下）：管理员、系统账号，和登记过的人（停用了的也算：用过的照旧算数）。
    pub(crate) fn usage_accounts(&self) -> Vec<AccountId> {
        let mut accounts = self.accounts();
        for person in crate::login::members::people(self) {
            if !accounts.contains(&person) {
                accounts.push(person);
            }
        }
        accounts
    }

    /// 这次起来认的带进程的包的账号（施工 Y-5 中上），照编号排：平台桥在里面。
    pub(crate) fn package_accounts(&self) -> Vec<AccountId> {
        miyu_store::packages::package_accounts(&self.system_packages())
    }

    /// 连接是谁：核心拉起的带进程的包 `package` 是它自己的账号（施工 Y-5 中上），别的是管理员（本机连上来的都是他）。
    pub(crate) fn account_of(&self, package: Option<&str>) -> AccountId {
        package
            .and_then(|package| AccountId::parse(package).ok())
            .filter(|account| self.package_accounts().contains(account))
            .unwrap_or_else(|| self.admin.clone())
    }

    /// 包的账号 `account` 替谁干活（施工 Y-5 中上，设计 06 U17）：没声明系统账号的包替机主，交机主；平台桥替自己、桥对着的
    /// 外人，和别的连接一样没有（照身份）。
    pub(crate) fn serving(&self, account: &AccountId) -> Option<AccountId> {
        (self.package_accounts().contains(account) && !self.is_system(account))
            .then(|| self.admin.clone())
    }

    /// 属主是 `owner` 的会话，记忆归谁（`personas.md`「怎么走」第 5 条，施工 Y-7 起不看人格住在谁家）：系统账号的归管理员（机主），
    /// 别的归属主自己。
    pub(crate) fn memory_owner(&self, owner: &AccountId) -> AccountId {
        match self.is_system(owner) {
            true => self.admin.clone(),
            false => owner.clone(),
        }
    }
}

/// 建带进程的包的账号的家目录和工作区（已经有的不动；施工 Y-5 中上起不只平台桥），开它们各自的会话列表的索引。建不成的记一行，账号照样算有：用到时再报。
/// 起来时调一次，装卸以后再调（施工 F-5 下）：只管还没开索引的，开过的什么都不做。
pub(crate) fn prepare(core: &Core) {
    let mut indexes = core
        .indexes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for account in core.package_accounts() {
        if indexes.contains_key(&account) {
            continue;
        }
        if let Err(error) = core.root.prepare_home(&account) {
            tracing::warn!(target: TARGET, account = account.as_str(), kind = ?error.kind(), "system account not prepared");
        }
        let index = Arc::new(crate::list::open_index(&core.root, &account));
        indexes.insert(account, index);
    }
}

#[cfg(test)]
mod tests;

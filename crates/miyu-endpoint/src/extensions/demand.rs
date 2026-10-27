//! 用到时拉起（施工 IM-1 中，接入通讯平台设计 06 第 4.6 节第 1 样）：每人一份的包不在核心起来时全起，她替谁调到它的工具时才替
//! 那个人起一份。起之前照开机时那几道查：开关开着、依赖的包都在、程序在、要的能力都批了；再加一道：他在管理员组，或者包勾了
//! 「成员也能用」。包自己的账号（平台桥这些）不替它们起。

use std::sync::Arc;

use super::{Instance, approval, on, per_account, read_switches};
use crate::Core;

impl Core {
    /// 替 `instance` 那个人拉起每人一份的那一份：在跑的不动。交回他能不能用、这一次起没起（或者本来就在跑）；不能用的不起。
    pub(crate) fn start_for(self: &Arc<Self>, instance: &Instance) -> bool {
        let Some(account) = &instance.account else {
            return false;
        };
        if self.package_accounts().contains(account) {
            return false;
        }
        let id = instance.package.as_str();
        let packages = self.system_packages();
        let all: Vec<&miyu_store::packages::Found> = packages.iter().collect();
        let Some(manifest) = packages
            .iter()
            .find(|found| found.id == id)
            .and_then(|found| found.read.as_ref().ok())
        else {
            return false;
        };
        let (switches, _) = read_switches(self);
        let usable = per_account(manifest)
            && on(&switches, id, manifest)
            && (crate::login::admins::is_admin(self, account) || self.members_may(id, manifest))
            && crate::packages::depends::missing(&all, manifest).is_empty()
            && !crate::packages::program_missing(self, id, manifest)
            && approval::unapproved(self, id, manifest).is_empty();
        if usable {
            self.extensions.launch(self, instance, manifest);
        }
        usable
    }
}

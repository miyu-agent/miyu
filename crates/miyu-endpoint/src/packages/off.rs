//! 每个人把某个包对自己整个关掉（施工 U23 末上，设计 06 U23，项目主人 2026-10-24）：个人设置 `packages.<编号>.off`
//! （`miyu_session::packages_off`）。他看得到的包（[`Core::packages_of`]）去掉他关了的：技能、人格预设、命令模板、交给会话的「装了
//! 的包」、预设页都跟着没有；会话每一轮照活的配置把关了的包并进预设文件扣下的那几个（`miyu_policy::preset::PresetFile::switching_off`），
//! 工具面、技能下一轮就没有。施工 BS-1 上中起没有必需的包，哪个都关得掉。包的账号（平台桥、替机主干活的扩展）不看谁的个人
//! 设置。
//!
//! 成员也能用（施工 IM-1 上，设计 06 第八再又补第 2 条）：带进程的包（扩展、带 MCP 服务的）默认只有管理员组的人能用。不在管理员组的
//! 人，没勾「成员也能用」的这几个包同样当他关了。勾没勾照系统那一层的开关（`system/extensions.json` 的 `members`），没改过的照
//! 清单的 `[package] members`；只认系统装的包，成员自己装不了带进程的。

use std::collections::BTreeSet;

use miyu_config::package::Manifest;
use miyu_kernel::id::AccountId;

use crate::Core;
use crate::extensions::read_switches;
use crate::packages::status::runs;

impl Core {
    /// `account` 关了的包：照他那一份配置读，不在管理员组的再加上成员用不了的；包的账号的是空的。
    pub(crate) fn packages_off(&self, account: &AccountId) -> BTreeSet<String> {
        if self.package_accounts().contains(account) {
            return BTreeSet::new();
        }
        let values = self.mine(account).config().resolved().values();
        let mut off = miyu_session::packages_off::of(&values);
        if !crate::login::is_owner(self, account) {
            off.extend(self.barred_from_members());
        }
        off
    }

    /// 成员用不了的系统装的包：带进程、没勾「成员也能用」的。
    pub(crate) fn barred_from_members(&self) -> Vec<String> {
        self.system_packages()
            .iter()
            .filter_map(|found| {
                let manifest = found.read.as_ref().ok()?;
                (runs(manifest) && !self.members_may(&found.id, manifest)).then(|| found.id.clone())
            })
            .collect()
    }

    /// 包 `id`（清单 `manifest`）成员能不能用：管理员在系统那一层改过的照他的，没改过的照清单。
    pub(crate) fn members_may(&self, id: &str, manifest: &Manifest) -> bool {
        let (switches, _) = read_switches(self);
        switches
            .members
            .get(id)
            .copied()
            .unwrap_or(manifest.members)
    }
}

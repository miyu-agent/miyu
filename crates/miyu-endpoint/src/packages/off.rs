//! 每个人把某个包对自己整个关掉（施工 U23 末上，设计 06 U23，项目主人 2026-10-24）：个人设置 `packages.<编号>.off`
//! （`miyu_session::packages_off`）。他看得到的包（[`Core::packages_of`]）去掉他关了的：技能、人格预设、命令模板、交给会话的「装了
//! 的包」、预设页都跟着没有；会话每一轮照活的配置把关了的包并进预设文件扣下的那几个（`miyu_policy::preset::PresetFile::switching_off`），
//! 工具面、技能下一轮就没有。施工 BS-1 上中起没有必需的包，哪个都关得掉。包的账号（平台桥、替机主干活的扩展）不看谁的个人
//! 设置。

use std::collections::BTreeSet;

use miyu_kernel::id::AccountId;

use crate::Core;

impl Core {
    /// `account` 关了的包：照他那一份配置读；包的账号的是空的。
    pub(crate) fn packages_off(&self, account: &AccountId) -> BTreeSet<String> {
        if self.package_accounts().contains(account) {
            return BTreeSet::new();
        }
        let values = self.mine(account).config().resolved().values();
        miyu_session::packages_off::of(&values)
    }
}

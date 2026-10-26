//! 每个人把某个包对自己整个关掉（施工 U23 末上，设计 06 U23，项目主人 2026-10-24）：个人设置 `packages.<编号>.off`
//! （`miyu_session::packages_off`）。他看得到的包（[`Core::packages_of`]）去掉他关了的：技能、人格预设、命令模板、交给会话的「装了
//! 的包」、预设页都跟着没有；会话每一轮照活的配置把关了的包并进预设文件扣下的那几个（`miyu_policy::preset::PresetFile::switching_off`），
//! 工具面、技能下一轮就没有。必需的包关不了：`config.set` 拒，手改写了的当没写。包的账号（平台桥、替机主干活的扩展）不看谁的个人
//! 设置。

use std::collections::BTreeSet;

use miyu_config::Value;
use miyu_config::key;
use miyu_config::package::settings::OFF;
use miyu_config::problem::Code;
use miyu_kernel::id::AccountId;
use miyu_store::packages::Found;

use crate::Core;

/// 必需的包（清单写了 `[package] required`）的编号。
pub(crate) fn required(found: &[Found]) -> BTreeSet<String> {
    found
        .iter()
        .filter(|one| one.read.as_ref().is_ok_and(|manifest| manifest.required))
        .map(|one| one.id.clone())
        .collect()
}

/// `config.set` 多查的一道：把必需的包 `required` 里的写成关着（`packages.<编号>.off = true`）的不收，照选项不对说。别的没有。
pub(crate) fn problem(
    real: &str,
    value: &Value,
    required: &BTreeSet<String>,
) -> Option<(Code, String)> {
    if !matches!(value, Value::Bool(true)) {
        return None;
    }
    match key::fit(OFF, &key::split(real)?) {
        key::Fit::Yes(names) if names.iter().any(|id| required.contains(id)) => {
            Some((Code::NotAnOption, "true".to_string()))
        }
        _ => None,
    }
}

impl Core {
    /// `account` 关了的包：照他那一份配置读，去掉这一刻必需的；包的账号的是空的。
    pub(crate) fn packages_off(&self, account: &AccountId) -> BTreeSet<String> {
        if self.package_accounts().contains(account) {
            return BTreeSet::new();
        }
        let values = self.mine(account).config().resolved().values();
        let required = required(&self.system_packages());
        miyu_session::packages_off::of(&values)
            .into_iter()
            .filter(|id| !required.contains(id))
            .collect()
    }
}

#[cfg(test)]
mod tests;

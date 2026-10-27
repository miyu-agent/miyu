//! 一份扩展进程是谁（施工 IM-1 中）：哪个包，替谁跑。一份的（`[process] scope = "system"`，默认）没有账号，用包自己的账号跑，
//! 和以前一样；每人一份的（`scope = "account"`）带上那个人，以他的身份连进来，数据放在他自己的家目录里。

use std::path::PathBuf;

use miyu_config::package::{Manifest, Scope};
use miyu_kernel::id::AccountId;

use crate::Core;

/// 一份扩展进程。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Instance {
    /// 包的编号。
    pub(crate) package: String,
    /// 每人一份的那个人；一份的没有。
    pub(crate) account: Option<AccountId>,
}

impl Instance {
    /// 包 `package` 的那一份（一份的）。
    pub(crate) fn system(package: &str) -> Instance {
        Instance {
            package: package.to_string(),
            account: None,
        }
    }

    /// 运行日志、标准错误的文件名里写的：一份的是包的编号，每人一份的是 `<编号>.<账号>`（账号的写法里没有点）。
    pub(crate) fn label(&self) -> String {
        match &self.account {
            None => self.package.clone(),
            Some(account) => format!("{}.{}", self.package, account.as_str()),
        }
    }

    /// 放状态的目录：一份的是 `<数据根>/state/packages/<编号>/`，每人一份的是 `home/<账号>/state/packages/<编号>/`。不建。
    pub(crate) fn state_dir(&self, core: &Core) -> PathBuf {
        match &self.account {
            None => crate::packages::packages(core).state_dir(&self.package),
            Some(account) => core
                .root
                .account_dir(account)
                .join("state")
                .join("packages")
                .join(&self.package),
        }
    }
}

/// 清单说每人一份。
pub(crate) fn per_account(manifest: &Manifest) -> bool {
    manifest
        .process
        .as_ref()
        .is_some_and(|process| process.scope == Scope::Account)
}

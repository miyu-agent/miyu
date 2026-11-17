//! 装着的包（施工 F-5 上）：[`Core::system_packages`] 是出厂的、装上的全部，进程、扩展、装卸、会话的工具面、技能、预设都照它。
//! 施工 UX-1 起没有「只对某个人关掉的包」（`packages.<编号>.off` 删了），关一个包就是 `package.disable`。

use std::sync::{Arc, PoisonError};

use miyu_store::packages::Found;

use crate::Core;

impl Core {
    /// 装着的软件包（施工 F-5 上：装卸以后当场换）：出厂的、装上的全部。
    pub fn system_packages(&self) -> Arc<Vec<Found>> {
        Arc::clone(&self.packages.read().unwrap_or_else(PoisonError::into_inner))
    }
}

//! 一份扩展进程是谁（施工 IM-1 中；SU-2 下起每个包只有一份）：哪个包。

use std::path::PathBuf;

use crate::Core;

/// 一份扩展进程。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Instance {
    /// 包的编号。
    pub(crate) package: String,
}

impl Instance {
    /// 包 `package` 的那一份。
    pub(crate) fn of(package: &str) -> Instance {
        Instance {
            package: package.to_string(),
        }
    }

    /// 运行日志、标准错误的文件名里写的：包的编号。
    pub(crate) fn label(&self) -> String {
        self.package.clone()
    }

    /// 放状态的目录：`<数据根>/state/packages/<编号>/`。不建。
    pub(crate) fn state_dir(&self, core: &Core) -> PathBuf {
        crate::packages::packages(core).state_dir(&self.package)
    }
}

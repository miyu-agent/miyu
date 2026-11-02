//! 一份扩展进程是谁（施工 IM-1 中；SU-2 下起每个包只有一份）：哪个包。

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
}

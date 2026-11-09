//! 顾问会话的 system（施工 ADV-1，设计 `69-旗舰顾问.md` 第三节第 4 条）：造顾问会话时读一次，进它的快照；之后照快照发，
//! 接着聊时前缀不变。

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 顾问会话的 system：`core/advisor/system.txt` 的原文。
    ///
    /// # Errors
    ///
    /// 读不了这份文件，写明是哪一份。
    pub fn advisor_system(&self) -> Result<String, SourceError> {
        self.read(&["core", "advisor", "system.txt"])
    }
}

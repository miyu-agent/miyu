//! 扩展放在系统提示词尾部的那几段（施工 X-20，`docs/designs/34-表情包.md` 第五节第 5 条）：扩展经 `system.append` 交来，每个包
//! 一段，原文照抄（外壳、转义扩展自己做），照包的编号排，接在整段 system 的最后，记进快照。只插不改人格原文。

use std::collections::BTreeMap;

use crate::Snapshot;
use crate::compose::system;

/// 一段最多几个字。
pub const APPEND_MAX: usize = 4000;

impl Snapshot {
    /// 带上扩展放的那几段（施工 X-20）：键是包的编号，照编号排、块间空一行，接在最后（[`Snapshot::with_style_lock`] 以后）；
    /// 记进快照。没有的 system 一字不变。
    #[must_use]
    pub fn with_appended(mut self, appended: BTreeMap<String, String>) -> Snapshot {
        for text in appended.values() {
            self.system = system(&[&self.system, text]);
        }
        self.appended = appended;
        self
    }
}

#[cfg(test)]
mod tests;

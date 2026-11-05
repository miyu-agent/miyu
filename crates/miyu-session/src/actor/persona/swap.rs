//! 换快照时一起交出去的几样（施工 P-13、P-1 三补；PX-1 从 `persona.rs` 挪出来）。

use std::sync::Arc;

use miyu_drivers::DriverTexts;
use miyu_policy::{GuardTexts, RunTexts};

use crate::knowledge::Shelves;

/// 换了人格（施工 P-13）：快照换上以后 actor 照它挪记忆、换知识库。
pub(super) struct Recast {
    /// 新人格的知识库：没接知识库的没有。
    pub(super) shelves: Option<Arc<Shelves>>,
}

/// 执行器取自快照的几份字（施工 P-1 三补）：换快照时一起换。
pub(super) struct Retext {
    /// 驱动的占位：交给请求模型的端口。
    pub(super) driver: DriverTexts,
    /// 执行工具时替工具写的两句。
    pub(super) run: RunTexts,
    /// 权限策略的几句。
    pub(super) guard: GuardTexts,
}

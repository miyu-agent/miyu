//! 这一轮的配置的端口（施工 N-2 下，设计 36 第二节第 3 条）：会话 actor 在回合开始冻结的那一份，工具照它读自己包的配置项、
//! 照引用取密钥。会话交进 [`crate::Call`]；测试里的假调用、会话外面的调用没有，工具照默认值办。

use miyu_config::Values;
use miyu_config::secret::{Reference, Secret};

/// 这一轮冻结的配置。
pub trait SettingsPort: Send + Sync {
    /// 最终值（带上信任着的项目配置合出来的）。
    fn values(&self) -> Values;

    /// 照引用取一个密钥：这一轮开始时的那一份。没设的、设成空的是空的。
    fn secret(&self, reference: &Reference) -> Option<Secret>;
}

impl std::fmt::Debug for dyn SettingsPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SettingsPort")
    }
}

/// 同一份才算相等：调用比的是带没带同一个端口。
impl PartialEq for dyn SettingsPort {
    fn eq(&self, other: &dyn SettingsPort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn SettingsPort {}

//! 一份配置（施工 SU-3 上）：[`Core::mine`] 交回核心手里那一份。

use miyu_session::Configs;

use super::Config;
use super::hub::{Hub, Pushing};

use crate::Core;

/// 配置：核心手里那一份。
pub(crate) struct Mine<'a>(&'a Core);

impl Mine<'_> {
    /// 配置服务：同 [`Core::config`]，拿着的时候不许 `.await`。
    pub(crate) fn config(&self) -> std::sync::MutexGuard<'_, Config> {
        self.0.config()
    }

    /// 换了交给谁：会话、订阅着配置的连接。
    pub(crate) fn hub(&self) -> &Hub {
        &self.0.hub
    }
}

impl Core {
    /// 配置：只有一份（施工 SU-3 上）。
    pub(crate) fn mine(&self) -> Mine<'_> {
        Mine(self)
    }

    /// 会话取配置的那一头：造会话、载入时交给它。
    pub(crate) fn configs_of(&self) -> Configs {
        self.hub.configs()
    }
}

/// 配置服务换上了 `config`：交给会话、订阅着的连接（同 [`Hub::publish`]）。拿着 `config` 的锁调。
pub(crate) fn publish(core: &Core, config: &Config, pushing: Option<Pushing>) {
    core.hub.publish(config, pushing);
}

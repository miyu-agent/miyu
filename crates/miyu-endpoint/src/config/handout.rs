//! 配置换了交给谁（施工 SU-3 上起只有一份配置）：会话取配置的那一头、换上了以后推给会话和订阅着的连接。

use miyu_session::Configs;

use super::Config;
use super::hub::Pushing;

use crate::Core;

impl Core {
    /// 会话取配置的那一头：造会话、载入时交给它。
    pub(crate) fn configs_of(&self) -> Configs {
        self.hub.configs()
    }
}

/// 配置服务换上了 `config`：交给会话、订阅着的连接（同 [`super::hub::Hub::publish`]）。拿着 `config` 的锁调。
pub(crate) fn publish(core: &Core, config: &Config, pushing: Option<Pushing>) {
    core.hub.publish(config, pushing);
}

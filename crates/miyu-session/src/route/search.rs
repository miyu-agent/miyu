//! 供应商自己的联网工具在路由这一头（设计 `72-DeepSeek内置联网工具.md` 第二节，施工 DSR-3）。
//!
//! - 回合开始解析完引用，记下模型这边给不给得了（[`Routes::search_offer`]）：一个模型照它自己，池要每个成员都给得了、给的是
//!   同一份（出错会换到别的成员，同看不了图的算法）。会话 actor 照它和快照里钉的判断要不要换工具面。
//! - 快照钉着要声明的那几个名字由 actor 交来（[`Held::declare`]）。发请求时这个端点自己给得了的才声明（[`Held::declared_on`]）：
//!   出错换到了给不了的端点的那一次不声明，免得别家接口收到认不得的工具。

use std::sync::{Mutex, PoisonError};

use miyu_models::facts::facts;
use miyu_models::pools::Pool;
use miyu_models::provider::Target;
use miyu_models::provider_search::{ProviderSearch, offered};
use miyu_models::reference::Resolved;

use super::Routes;
use super::base::Ready;
use crate::config::TurnConfig;

/// 一个会话记着的两样。
#[derive(Default)]
pub(super) struct Held {
    /// 回合开始时模型这边给得了的那一份；给不了的、还没开过回合的没有。
    offer: Mutex<Option<ProviderSearch>>,
    /// 快照钉着要声明的供应商侧工具名，照先后。
    declared: Mutex<Vec<String>>,
}

impl Held {
    /// 回合开始：换成这一刻的。
    pub(super) fn offer(&self, offer: Option<ProviderSearch>) {
        *self.offer.lock().unwrap_or_else(PoisonError::into_inner) = offer;
    }

    /// 这一刻给得了的，写成策略那边认的样子。
    pub(super) fn offered(&self) -> Option<miyu_policy::SearchOffer> {
        let offer = self.offer.lock().unwrap_or_else(PoisonError::into_inner);
        offer.as_ref().map(|search| miyu_policy::SearchOffer {
            setting: search.setting.clone(),
            replaces: search.replaces.clone(),
        })
    }

    /// 换快照了、载入了：要声明的换成 `hosted`。
    pub(super) fn declare(&self, hosted: Vec<String>) {
        *self.declared.lock().unwrap_or_else(PoisonError::into_inner) = hosted;
    }

    /// 这一次发给 `ready` 的端点：它给得了的带上要声明的，给不了的照原样。声明了、这一次又没有定思考强度的，照档案写的
    /// 那一档明着写（不明着写，DeepSeek 不会真去搜索），空闲超时 `idle` 跟着那一档放大。
    pub(super) fn declared_on(&self, mut ready: Ready, idle: std::time::Duration) -> Ready {
        ready.call.hosted = self.hosted(ready.search.is_some());
        let level = explicit(
            &ready.call.hosted,
            ready.call.effort.as_deref(),
            ready.search.as_deref(),
        );
        if let Some(level) = level {
            ready.idle = super::effort::idle(idle, Some(&level));
            ready.call.effort = Some(level);
        }
        ready
    }

    /// 发给一个端点时要声明的：这个端点给得了（`search`）的是快照钉着的那几个，给不了的是空的。
    fn hosted(&self, search: bool) -> Vec<String> {
        match search {
            true => self
                .declared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            false => Vec::new(),
        }
    }
}

/// 这一次要明着写的思考强度：声明了供应商侧工具 `hosted`、这一次又没有定强度（`effort` 没有）的，是档案写的那一档
/// `level`；别的不改，没有。
fn explicit(hosted: &[String], effort: Option<&str>, level: Option<&str>) -> Option<String> {
    match (hosted.is_empty(), effort) {
        (false, None) => level.map(str::to_string),
        _ => None,
    }
}

impl Routes {
    /// 引用 `resolved` 指到的模型给不给得了：会话记的思考强度 `effort` 只算在单个模型上（池的成员各照各的配置）。
    pub(super) fn search_offer(
        &self,
        config: &TurnConfig,
        resolved: &Resolved,
        effort: Option<&str>,
    ) -> Option<ProviderSearch> {
        match resolved {
            Resolved::Model(target) => self.target_offer(config, target, effort),
            Resolved::Pool(pool) => self.pool_offer(config, pool),
        }
    }

    /// 池的每个成员都给得了、给的是同一份才算。
    fn pool_offer(&self, config: &TurnConfig, pool: &Pool) -> Option<ProviderSearch> {
        let values = config.resolved.values();
        let mut offers = pool.members.iter().map(|member| {
            let target = super::pool::target(&self.data, &values, member).ok()?;
            self.target_offer(config, &target, None)
        });
        let first = offers.next()??;
        offers
            .all(|offer| offer.as_ref() == Some(&first))
            .then_some(first)
    }

    /// 发给 `target` 这一个模型时给不给得了：驱动、地址照真发的那个模型（[`miyu_models::provider::Provider::for_model`]），
    /// 思考强度照会话记的（这个模型有的）、再照配置的默认，和 [`Routes::ready`] 一样。
    fn target_offer(
        &self,
        config: &TurnConfig,
        target: &Target,
        session: Option<&str>,
    ) -> Option<ProviderSearch> {
        let (facts, speaking) = self.data.with(|knowledge| {
            let (facts, _) = facts(&config.resolved, knowledge, &target.provider, &target.model);
            let speaking =
                target
                    .provider
                    .for_model(&target.model, &facts.wire, &knowledge.profiles.npm);
            (facts, speaking)
        });
        let speaking = speaking.ok()?;
        let effort = miyu_models::effort::held(session, facts.levels())
            .map(str::to_string)
            .or_else(|| facts.effort.value.clone());
        offered(&speaking, effort.as_deref()).cloned()
    }
}

#[cfg(test)]
mod tests;

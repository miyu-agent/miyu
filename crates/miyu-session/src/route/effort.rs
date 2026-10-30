//! 思考强度（施工 8-18；会话那一层 8-18（补）去掉、SM-1 加回来，设计 51，`docs/blueprint/models.md`「怎么走」第十一条第 4、
//! 6、7 条）。
//!
//! - 每次请求挑好端点以后，照真发的那个模型：会话记的一档它有的照会话的，不然照配置的默认（`route/base.rs`，
//!   `miyu_models::effort::held`）。
//! - 空闲超时照那一档放大（`miyu_models::effort::idle_factor`）。
//! - 给头看的那一档：限额里的那个模型（轮换的池没有），照端口记着的配置、会话的一档现算，连同从哪来（会话，或者配置的
//!   哪一层，`miyu_models::effort::shown`）。
//! - 限额里那一块事实写的档位（施工 E-M）同上：会话的一档这个模型有的，换成它（[`Routes::stamp`]）。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::event::EffortInUse;
use miyu_kernel::origin::Model;
use miyu_kernel::session::Limits;
use miyu_models::effort;
use miyu_models::facts::{Facts, facts};
use miyu_models::provider;

use super::{NONE, Route, Routes};
use crate::config::TurnConfig;

impl Route {
    /// 给头看的那一档：限额里的那个模型照记着的配置、会话的一档算。轮换的池、还没有模型的、那一家这时用不了的没有。
    pub(super) fn shown_effort(&self) -> Option<EffortInUse> {
        let pinned = self.lock();
        let model = pinned.limits.model.clone();
        let config = Arc::clone(&pinned.config);
        let session = pinned.effort.clone();
        drop(pinned);
        let facts = self.shared.facts_of(&config, &model)?;
        effort::shown(session.as_deref(), &facts.effort, facts.levels())
    }
}

impl Routes {
    /// 限额里的模型 `model` 照配置 `config` 的资料：还没有模型的（`none`）、那一家这时用不了的没有。
    fn facts_of(&self, config: &TurnConfig, model: &Model) -> Option<Facts> {
        if model.endpoint.as_str() == NONE {
            return None;
        }
        let values = config.resolved.values();
        self.data.with(|knowledge| {
            let provider = provider::provider(&values, knowledge, model.endpoint.as_str()).ok()?;
            Some(facts(&config.resolved, knowledge, &provider, model.model.as_str()).0)
        })
    }

    /// 限额里那一块事实写的档位换成会话记的 `session`（施工 SM-1）：限额里的模型有这一档的才换，别的照配置的不动。
    pub(super) fn stamp(&self, config: &TurnConfig, limits: &mut Limits, session: Option<&str>) {
        if session.is_none() || limits.identity.is_none() {
            return;
        }
        let Some(facts) = self.facts_of(config, &limits.model) else {
            return;
        };
        if let (Some(level), Some(identity)) = (
            effort::held(session, facts.levels()),
            limits.identity.as_mut(),
        ) {
            identity.effort = level.to_string();
        }
    }
}

/// 这一次的空闲超时：基数照那一档放大。
pub(super) fn idle(base: Duration, level: Option<&str>) -> Duration {
    base.saturating_mul(effort::idle_factor(level))
}

//! 换了还没开回合的给头看什么（施工 MC-1，2026-11-16 项目主人定「当场报新的」，`docs/blueprint/models.md`「怎么走」第六条
//! 第 3 条第 5 款）：人 `session.configure` 换了模型、思考强度，真发请求照旧下一个回合开始照新的（[`Route::begin`]）；`subscribe`
//! 的 `model`、`model.changed` 当场照会话记着的报。
//!
//! 照下一个回合开始会怎么解析预演一遍（同一个 [`settle`]：改过名的照新编号，解析不出的退回 `models.chat`），只查、不改钉着的：
//! 池的指针、钉着的成员、限额都不动。
//!
//! - 和钉着的一样（引用一样，思考强度也一样；池不用会话那一档，只比引用）：没有，照钉着的报（钉住的池照钉着的成员）。
//! - 换成了模型的：那一家的那个模型，思考强度照这一刻的配置、会话记的那一档算（同 [`Route::shown_effort`]），算不出的不带。
//! - 换成了池的：只有引用，没有端点、模型、思考强度，下一个回合开始钉了成员才有（照「轮换的池、解析不出的没有」）。
//! - 都解析不出的：只有引用。

use miyu_models::effort;
use miyu_models::provider;
use miyu_models::reference::Resolved;

use super::turn::settle;
use super::{Route, model_of};
use crate::config::TurnConfig;
use crate::shown::Next;

impl Route {
    /// 照这一刻的配置 `config` 预演：`recorded` 是内核记着的引用（没记的跟着 `models.chat`），`effort` 是内核记着的思考强度。
    /// 和钉着的一样的没有（见模块的说明）。
    pub(super) fn foresee(
        &self,
        config: &TurnConfig,
        recorded: Option<&str>,
        effort: Option<&str>,
    ) -> Option<Next> {
        let values = config.resolved.values();
        let wanted = recorded
            .map(str::to_string)
            .or_else(|| provider::chat(&values));
        let pool = wanted.as_deref().is_some_and(|text| text.starts_with('@'));
        {
            let pinned = self.lock();
            if pinned.reference == wanted && (pool || pinned.effort.as_deref() == effort) {
                return None;
            }
        }
        let settled = settle(&self.shared.data, &values, wanted.as_deref());
        let reference = settled.to.or(wanted);
        let Some(Resolved::Model(target)) = settled.resolved else {
            return Some(Next {
                reference,
                model: None,
                effort: None,
            });
        };
        let model = model_of(&target);
        let effort = self
            .shared
            .facts_of(config, &model)
            .and_then(|facts| effort::shown(effort, &facts.effort, facts.levels()));
        let model = (model.endpoint.as_str() != super::NONE).then_some(model);
        Some(Next {
            reference,
            effort: model.as_ref().and(effort),
            model,
        })
    }
}

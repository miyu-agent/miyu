//! 回合开始时重新解析（施工 8-10，`internal/blueprint/models.md`「怎么走」第六条第 3、4 条）：照这一轮冻结的配置解析会话的
//! 引用，端点、限额换成解析出的。
//!
//! - 内核交了引用的，换成它（变了的，钉着的成员清掉）；没交的（人回到了跟着配置，以前的日志、造的时候连 `models.chat` 都
//!   没配的）照这一轮的 `models.chat`（施工 8-10 再补；以前照路由记在内存里的）。
//! - 解析不出、供应商改过名的（施工 8-34，`state/models/renamed.json`）：换成新编号再解析，解析出来的照下一条告诉内核
//!   （`replaced` 是旧的），记一行 `INFO model renamed`；换了也解析不出的照下一条。
//! - 解析不出的（供应商、池删了，池空了），退回这一轮的 `models.chat`，以后钉在它上面，记一行 `INFO model fallback`；内核
//!   交了引用的，交回原来的和退回的，内核记一条 `session.policy_changed`。`models.chat` 也解析不出的不退，端口是「没有
//!   模型」，这一轮的请求当场 `no_model`。
//! - 钉住的池：钉着的成员还在池里的照旧，不在了、刚换过来的取指针指的（`route/pool.rs`）；会话换过去的 key 照旧
//!   （`route/choice.rs` 照名字认，不在了的照会话编号钉着的）。
//! - 限额照解析出的重算：配置里改了窗口、用出来的窗口，这一轮就用上。
//! - 这一轮的配置记下（施工 8-18），会话记的思考强度换成内核交的（施工 SM-1）：每次请求、给头看的那一档、限额里那一块
//!   事实写的档位照它们算（`route/effort.rs`）。

use std::sync::Arc;

use miyu_config::Values;
use miyu_kernel::id::{ModelName, ProviderId};
use miyu_kernel::origin::Model;
use miyu_kernel::session::{Limits, Replaced};
use miyu_models::facts::facts;
use miyu_models::pools::Member;
use miyu_models::provider::{self, Target};
use miyu_models::reference::{Resolved, resolve};

use super::{ModelData, Route, Routes, images, nothing};
use crate::TARGET;
use crate::config::TurnConfig;

impl Route {
    /// 回合开始：照这一轮的配置 `config` 重新解析，`recorded` 是内核记着的引用，`effort` 是内核记着的思考强度（见模块的
    /// 说明）。交回退回了默认的那一次。
    pub(super) fn begin(
        &self,
        config: &TurnConfig,
        recorded: Option<&str>,
        effort: Option<&str>,
    ) -> Option<Replaced> {
        let values = config.resolved.values();
        let data = &self.shared.data;
        let mut pinned = self.lock();
        pinned.config = Arc::clone(config);
        // 内核没记引用的跟着这一轮的 `models.chat`（施工 8-10 再补：人能回到「跟着配置」）：配置换了，这一轮就换。
        let wanted = match recorded {
            Some(recorded) => Some(recorded.to_string()),
            None => provider::chat(&values),
        };
        if pinned.reference != wanted {
            pinned.reference = wanted;
            pinned.member = None;
        }
        let mut replaced = None;
        let settled = settle(data, &values, pinned.reference.as_deref());
        if let Some(to) = settled.to {
            match settled.renamed {
                true => tracing::info!(target: TARGET, "model renamed"),
                false => {
                    if let Some(from) = &pinned.reference {
                        tracing::info!(target: TARGET, from = from.as_str(), to = to.as_str(), "model fallback");
                    }
                }
            }
            replaced = recorded.map(|from| Replaced {
                from: from.to_string(),
                to: to.clone(),
            });
            pinned.reference = Some(to);
            pinned.member = None;
        }
        let resolved = settled.resolved;
        // 池不用会话记的那一档（SM-1 补）：成员各照各的配置；换回模型又用上。
        let effort = effort.filter(|_| !matches!(resolved, Some(Resolved::Pool(_))));
        pinned.effort = effort.map(str::to_string);
        // 供应商自己的联网工具这一刻给不给得了（设计 72，`search.rs`）：actor 回合开始接着问。
        let offer = resolved.as_ref();
        self.search
            .offer(offer.and_then(|resolved| self.shared.search_offer(config, resolved, effort)));
        let (member, mut limits) = match resolved {
            Some(Resolved::Model(target)) => (None, self.shared.limits(config, &target)),
            Some(Resolved::Pool(pool)) => {
                let held = pinned.member.as_ref().and_then(model_of);
                self.shared.pool_start(config, &pool, held.as_ref())
            }
            None => (None, nothing()),
        };
        self.shared.stamp(config, &mut limits, effort);
        pinned.member = member;
        pinned.last = limits.model.clone();
        pinned.limits = limits;
        replaced
    }
}

/// 引用解析到哪（[`settle`]）：回合开始照它换（[`Route::begin`]），换了还没开回合的照它预演（施工 MC-1，`route/upcoming.rs`）。
pub(super) struct Settled {
    /// 换成的引用：供应商改过名的新编号、退回的 `models.chat`。照原样解析得出的、退不回去的没有。
    pub(super) to: Option<String>,
    /// `to` 是改过名的新编号（不是退回的）。
    pub(super) renamed: bool,
    /// 解析出的。都解析不出的没有。
    pub(super) resolved: Option<Resolved>,
}

/// 引用 `reference` 照这一轮的配置 `values` 解析到哪（见模块的说明）：照原样解析得出的照它；解析不出、供应商改过名的换成新
/// 编号；还解析不出的退回 `models.chat`，它也解析不出的不退。只查，不记、不改钉着的。
pub(super) fn settle(data: &ModelData, values: &Values, reference: Option<&str>) -> Settled {
    let resolve = |text: &str| data.with(|knowledge| resolve(values, knowledge, text)).ok();
    let renamed = reference
        .and_then(|text| renamed(data, text))
        .and_then(|text| resolve(&text).map(|resolved| (text, resolved)));
    let direct = reference.and_then(resolve);
    let (to, renamed, resolved) = match (direct, renamed) {
        (Some(resolved), _) => (None, false, Some(resolved)),
        (None, Some((to, resolved))) => (Some(to), true, Some(resolved)),
        (None, None) => {
            let chat = provider::chat(values);
            let fallback = chat.as_deref().and_then(resolve);
            match (fallback, chat) {
                (Some(fallback), Some(to)) => (Some(to), false, Some(fallback)),
                _ => (None, false, None),
            }
        }
    };
    Settled {
        to,
        renamed,
        resolved,
    }
}

/// 引用 `text` 指的供应商改过名的（施工 8-34）：换成新编号的引用；池、没改过名的没有。
fn renamed(data: &ModelData, text: &str) -> Option<String> {
    let (provider, model) = text.split_once('/')?;
    if provider.starts_with('@') {
        return None;
    }
    data.renamed(provider).map(|to| format!("{to}/{model}"))
}

/// 池里一个成员写成端点和模型：钉住的池照它认回钉着的那一个（[`super::Routes::pool_start`]）。
fn model_of(member: &Member) -> Option<Model> {
    Some(Model {
        endpoint: ProviderId::parse(&member.provider).ok()?,
        model: ModelName::parse(&member.model).ok()?,
    })
}

impl Routes {
    /// 发给 `target` 的限额：窗口、最大输出照模型资料，一张图怎么算照档案。
    pub(super) fn limits(&self, config: &TurnConfig, target: &Target) -> Limits {
        let (facts, _) = self
            .data
            .with(|knowledge| facts(&config.resolved, knowledge, &target.provider, &target.model));
        Limits {
            model: super::model_of(target),
            window: facts.window.value,
            max_output: facts.max_output.value,
            images: images(target.provider.images),
            blind: !facts.driver_inputs().images,
            identity: Some(super::identity::identity(config, target, &facts.effort)),
        }
    }
}

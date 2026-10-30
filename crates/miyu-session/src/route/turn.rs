//! 回合开始时重新解析（施工 8-10，`docs/blueprint/models.md`「怎么走」第六条第 3、4 条）：照这一轮冻结的配置解析会话的
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

use miyu_kernel::id::{ModelName, ProviderId};
use miyu_kernel::origin::Model;
use miyu_kernel::session::Replaced;
use miyu_models::pools::Member;
use miyu_models::provider;
use miyu_models::reference::{Resolved, resolve};

use super::{Route, nothing};
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
        let resolve = |text: &str| {
            data.with(|knowledge| resolve(&values, knowledge, text))
                .ok()
        };
        let mut pinned = self.lock();
        pinned.config = Arc::clone(config);
        pinned.effort = effort.map(str::to_string);
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
        let renamed = pinned
            .reference
            .as_deref()
            .and_then(|text| renamed(data, text))
            .and_then(|text| resolve(&text).map(|resolved| (text, resolved)));
        let direct = pinned.reference.as_deref().and_then(resolve);
        let resolved = match (direct, renamed) {
            (Some(resolved), _) => Some(resolved),
            (None, Some((to, resolved))) => {
                tracing::info!(target: TARGET, "model renamed");
                replaced = recorded.map(|from| Replaced {
                    from: from.to_string(),
                    to: to.clone(),
                });
                pinned.reference = Some(to);
                pinned.member = None;
                Some(resolved)
            }
            (None, None) => {
                let chat = provider::chat(&values);
                let fallback = chat.as_deref().and_then(resolve);
                if let (Some(_), Some(to)) = (&fallback, chat) {
                    if let Some(from) = &pinned.reference {
                        tracing::info!(target: TARGET, from = from.as_str(), to = to.as_str(), "model fallback");
                    }
                    replaced = recorded.map(|from| Replaced {
                        from: from.to_string(),
                        to: to.clone(),
                    });
                    pinned.reference = Some(to);
                    pinned.member = None;
                }
                fallback
            }
        };
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

/// 引用 `text` 指的供应商改过名的（施工 8-34）：换成新编号的引用；池、没改过名的没有。
fn renamed(data: &super::ModelData, text: &str) -> Option<String> {
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

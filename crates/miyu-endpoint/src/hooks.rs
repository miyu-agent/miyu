//! 回合开始问扩展要加的字（施工 X-16，`docs/blueprint/extensions.md`「回合开始的挂接点」）：会话定义的 [`HooksPort`]，这里照
//! 核心手上的清单、提供者实现。造会话、载入会话时交进去一份。
//!
//! 端口拿着核心的弱引用（同 `spawn.rs`）：核心没了（正在退出）的不问。在跑、`provide` 过的连接才问：扩展没批过的能力起不来
//! （`extensions/approval.rs`），在跑就是批过的。每人一份的包问会话属主那一份（施工 IM-1 中下三），没在跑的不为这一问拉起：
//! 每一轮都问，不能让一轮等它起来。

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::Capability;
use miyu_kernel::id::{AccountId, FactKind, ModuleId, SessionId};
use miyu_tool::{HooksPort, Inject, Injecting, TurnAsk};

use crate::Core;
use crate::reverse::Peer;

/// 一个扩展一轮至多交几块。
const MOST_BLOCKS: usize = 3;

/// 一块至多几个字符。
const MOST_CHARS: usize = 4000;

/// 交给会话的端口。
pub(crate) fn port(core: &Arc<Core>) -> Arc<dyn HooksPort> {
    Arc::new(Hooks(Arc::downgrade(core)))
}

/// 核心那一头。
struct Hooks(Weak<Core>);

impl HooksPort for Hooks {
    /// 扩展经 `system.append` 放的（施工 X-20），加 MCP 服务这时的说明（施工 X-39，键是 `mcp:<服务名>`）。
    fn appended(&self, session: &str) -> BTreeMap<String, Option<String>> {
        self.0
            .upgrade()
            .map(|core| {
                let mut appended = core.appends.of(session);
                appended.extend(crate::mcp::instructions::appended(&core));
                appended
            })
            .unwrap_or_default()
    }

    fn turn_start(&self, ask: TurnAsk) -> Injecting<'_> {
        Box::pin(async move {
            let Some(core) = self.0.upgrade() else {
                return Vec::new();
            };
            let owner = match SessionId::parse(&ask.session) {
                Ok(session) => core.sessions.owner(&core, &session).await,
                Err(_) => None,
            };
            let asked = asked(&core, &ask.tools, owner.as_ref());
            if asked.is_empty() {
                return Vec::new();
            }
            let mut params = json!({"session": ask.session, "venue": ask.venue.as_str()});
            if let Some(text) = ask.text {
                params["text"] = Value::String(text);
            }
            // 几个并行问、各自限时（`Timing::answer`），照包的编号收。
            let wait = core.extensions.timing().answer;
            let answers: Vec<_> = asked
                .into_iter()
                .map(|(package, peer)| {
                    let params = params.clone();
                    tokio::spawn(async move {
                        let answer = tokio::time::timeout(wait, peer.call("user.append", params));
                        taken(&package, answer.await)
                    })
                })
                .collect();
            let mut injects = Vec::new();
            for answer in answers {
                injects.extend(answer.await.unwrap_or_default());
            }
            injects
        })
    }
}

/// 要问的扩展，照包的编号排：声明了 `context.inject`、在跑、登记过工具，而且这一刻的工具面上有它的工具。每人一份的问 `owner`
/// 那一份；不知道属主的只问一份的。
fn asked(core: &Core, tools: &[String], owner: Option<&AccountId>) -> Vec<(String, Peer)> {
    let catalog = core.tools();
    let mut asked: Vec<(String, Peer)> = core
        .system_packages()
        .iter()
        .filter(|found| {
            found.read.as_ref().is_ok_and(|manifest| {
                manifest.process.as_ref().is_some_and(|process| {
                    process.capabilities.contains(&Capability::ContextInject)
                })
            })
        })
        .filter(|found| {
            tools
                .iter()
                .any(|tool| catalog.package_of(tool) == Some(found.id.as_str()))
        })
        .filter_map(|found| {
            let instance = match owner {
                Some(owner) => crate::extensions::Instance::serving(core, &found.id, owner),
                None => crate::extensions::Instance::system(&found.id),
            };
            Some((found.id.clone(), core.provided.peer_of(&instance)?))
        })
        .collect();
    asked.sort_by(|a, b| a.0.cmp(&b.0));
    asked.dedup_by(|a, b| a.0 == b.0);
    asked
}

/// `user.append` 的回应。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    content: Vec<Block>,
}

/// 交回的一块。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    kind: String,
    text: String,
}

/// 照写法收下一个扩展的回应：超时、断了、回错误的当没交，写法不对的整份不要。
fn taken(
    package: &str,
    answer: Result<Result<Result<Value, Value>, crate::reverse::Gone>, tokio::time::error::Elapsed>,
) -> Vec<Inject> {
    let result = match answer {
        Ok(Ok(Ok(result))) => result,
        Ok(Ok(Err(_))) | Ok(Err(_)) => {
            tracing::debug!(target: "miyu::endpoint", package, "user.append refused or gone");
            return Vec::new();
        }
        Err(_) => {
            tracing::debug!(target: "miyu::endpoint", package, "user.append timed out");
            return Vec::new();
        }
    };
    match blocks(package, result) {
        Some(blocks) => blocks,
        None => {
            tracing::warn!(target: "miyu::endpoint", package, "user.append answer malformed");
            Vec::new()
        }
    }
}

/// 写法都对的交回各块，有一处不对的交回 `None`。
fn blocks(package: &str, result: Value) -> Option<Vec<Inject>> {
    ModuleId::parse(package).ok()?;
    let answer: Answer = serde_json::from_value(result).ok()?;
    if answer.content.len() > MOST_BLOCKS {
        return None;
    }
    answer
        .content
        .into_iter()
        .map(|block| {
            FactKind::parse(&block.kind).ok()?;
            let chars = block.text.chars().count();
            (1..=MOST_CHARS).contains(&chars).then(|| Inject {
                package: package.to_string(),
                kind: block.kind,
                text: block.text,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;

//! 回合开始问扩展要加的字（施工 X-16，`docs/blueprint/extensions.md`「回合开始的挂接点」）：会话定义的 [`HooksPort`]，这里照
//! 核心手上的清单、提供者实现。造会话、载入会话时交进去一份。
//!
//! 端口拿着核心的弱引用（同 `spawn.rs`）：核心没了（正在退出）的不问。在跑、`provide` 过的连接才问：扩展没批过的能力起不来
//! （`extensions/approval.rs`），在跑就是批过的。

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::Capability;
use miyu_kernel::id::{FactKind, ModuleId};
use miyu_tool::{HooksPort, Inject, Injecting, TurnAsk};

use crate::Core;
use crate::reverse::Peer;

/// 每一个扩展答 `user.append` 最多等多久。
const HOOK_ANSWER: Duration = Duration::from_millis(300);

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
    fn appended(&self, session: &str) -> BTreeMap<String, Option<String>> {
        self.0
            .upgrade()
            .map(|core| core.appends.of(session))
            .unwrap_or_default()
    }

    fn turn_start(&self, ask: TurnAsk) -> Injecting<'_> {
        Box::pin(async move {
            let Some(core) = self.0.upgrade() else {
                return Vec::new();
            };
            let asked = asked(&core, &ask.tools);
            if asked.is_empty() {
                return Vec::new();
            }
            let mut params = json!({"session": ask.session, "venue": ask.venue.as_str()});
            if let Some(text) = ask.text {
                params["text"] = Value::String(text);
            }
            // 几个并行问、各自限时，照包的编号收。
            let answers: Vec<_> = asked
                .into_iter()
                .map(|(package, peer)| {
                    let params = params.clone();
                    tokio::spawn(async move {
                        let answer =
                            tokio::time::timeout(HOOK_ANSWER, peer.call("user.append", params));
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

/// 要问的扩展，照包的编号排：声明了 `context.inject`、在跑、登记过工具，而且这一刻的工具面上有它的工具。
fn asked(core: &Core, tools: &[String]) -> Vec<(String, Peer)> {
    let catalog = core.tools();
    let mut asked: Vec<(String, Peer)> = core
        .packages()
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
        .filter_map(|found| Some((found.id.clone(), core.provided.peer(&found.id)?)))
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

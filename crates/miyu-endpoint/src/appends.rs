//! `system.append {content, session?}`（施工 X-20，`docs/designs/34-表情包.md` 第五节第 5 条）：扩展往 system 尾部放一段。不写
//! `session` 的管它出现的全部会话，写了的只管那一个、优先；`content` 写 `null` 去掉那一层。只记在核心的内存里：会话在下一轮
//! 开头经端口问（`hooks.rs`），和快照里钉的不一样就换；核心重启以后照钉的，扩展再设一样的不再换。
//!
//! 只给声明了 `context.system` 的扩展：没批的扩展起不来（`extensions/approval.rs`），在跑就是批过的。

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::Capability;
use miyu_kernel::id::SessionId;
use miyu_policy::APPEND_MAX;

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// `system.append` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AppendParams {
    /// 要放的原文；`null` 去掉那一层。
    content: Option<String>,
    /// 只管哪个会话；不写的管全部。
    #[serde(default)]
    session: Option<String>,
}

/// 一个包放的。
#[derive(Default)]
struct Placed {
    /// 全部会话的那一份。
    all: Option<String>,
    /// 单给某个会话的，优先。
    sessions: HashMap<String, String>,
}

/// 各个包放的，照包的编号。
#[derive(Default)]
pub(crate) struct Appends(Mutex<BTreeMap<String, Placed>>);

impl Appends {
    /// 会话 `session` 这一刻该放的：说过话的每个包一项，`None` 是去掉了。
    pub(crate) fn of(&self, session: &str) -> BTreeMap<String, Option<String>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(package, placed)| {
                let text = placed.sessions.get(session).or(placed.all.as_ref());
                (package.clone(), text.cloned())
            })
            .collect()
    }

    /// 记下包 `package` 放的。
    fn set(&self, package: &str, session: Option<String>, content: Option<String>) {
        let mut placed = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let placed = placed.entry(package.to_string()).or_default();
        match (session, content) {
            (Some(session), Some(text)) => {
                placed.sessions.insert(session, text);
            }
            (Some(session), None) => {
                placed.sessions.remove(&session);
            }
            (None, text) => placed.all = text,
        }
    }
}

/// `system.append`：见模块的说明。
pub(crate) fn append(
    core: &Arc<Core>,
    caller: &Caller,
    params: AppendParams,
) -> Result<Value, Refusal> {
    let Some(package) = caller.package.as_deref() else {
        return Err(Refusal::NOT_A_PROVIDER);
    };
    let declared = core.packages().iter().any(|found| {
        found.id == package
            && found.read.as_ref().is_ok_and(|manifest| {
                manifest.process.as_ref().is_some_and(|process| {
                    process.capabilities.contains(&Capability::ContextSystem)
                })
            })
    });
    if !declared {
        return Err(Refusal::needs_approval(json!([
            Capability::ContextSystem.as_str()
        ])));
    }
    if let Some(session) = &params.session {
        SessionId::parse(session).map_err(|_| Refusal::BAD_PARAMS)?;
    }
    if let Some(content) = &params.content
        && !(1..=APPEND_MAX).contains(&content.chars().count())
    {
        return Err(Refusal::BAD_PARAMS);
    }
    core.appends.set(package, params.session, params.content);
    Ok(json!({}))
}

#[cfg(test)]
mod tests;

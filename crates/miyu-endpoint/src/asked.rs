//! `session.prompts {session}`（施工 SP-2，`docs/blueprint/protocol.md`「`session.prompts`」）：一个会话里主人问过的所有话，
//! 回应 `{"prompts": [{seq, turn, at, text}]}`，照先后。网页的「问话跳转」照它列出来，没读进来的先照 `seq` 用 `view.page` 的
//! `before` 往前读到那一页再跳。
//!
//! 口径同有效历史（留着一切的那一份，`History::whole`）：撤销掉的不列，恢复的放回，重做（编辑重发）照最后那一版，撤回的
//! 不列，压缩换出去的照样列。只列主人本人说的（`by` 是主人，桥代主人在私聊里转进来的、带 `via` 的也算）：子会话、父会话
//! 发来的，平台上别的人说的，模块注入的，别的 harness 的，定时的都不列。分叉出来的会话只列它自己日志里的：底子里的话在父会话的
//! 日志里，`view.page` 也翻不到。字照会话表的 `preview` 的取法（`miyu_store::index::said`），最多 [`CHARS`] 个字。
//!
//! 只读日志、不载入会话（同 `view.page`）；读法同判官（`crate::sweep`）：一遍读下来，只解要的种类（[`ONLY`]），撤销照真的
//! 有效历史算，前面谁都碰不到的那一截收下就放掉，不落缓存、不常驻。

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_store::log::{Flow, OpenError, read_range_only};

use crate::Core;
use crate::refusal::Refusal;
use crate::sweep::{Ended, Plan, Settle, Sweep, Tuning};

/// 一句问话的字最多几个字（按 Unicode 的字数）。
const CHARS: usize = 100;

/// 要全解的种类：人话。撤销、恢复、回合的开头和结尾这些有效历史照种类认的，[`read_range_only`] 自己也全解；别的只认外壳。
const ONLY: &[&str] = &["message.user"];

/// `session.prompts` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PromptsParams {
    /// 会话编号。
    session: String,
}

/// 一句问话。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Prompt {
    /// 那条 `message.user` 的序号。
    pub(crate) seq: Seq,
    /// 它的回合：回合里说的（排着的）是那一轮；闲着时说的是它开的那一轮；都没有的不写。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) turn: Option<TurnId>,
    /// 它的时刻。
    pub(crate) at: Timestamp,
    /// 第一行，最多 [`CHARS`] 个字。
    pub(crate) text: String,
}

/// `session.prompts`：见模块的说明。出错同 `view.page`：编号不合写法、多写了格的 `bad_params`；没有这个会话的
/// `session_not_found`；日志读不了的 `session_broken`。
pub(crate) async fn prompts(core: &Core, params: PromptsParams) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    if !core.sessions.exists(core, &session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let dir = core.root.session_dir(&session);
    let listed = tokio::task::spawn_blocking(move || list(&dir, Tuning::SHIPPED))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    match listed {
        Ok(prompts) => Ok(json!({ "prompts": prompts })),
        Err(OpenError::Missing(_)) => Err(Refusal::NOT_FOUND),
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "log not read");
            Err(Refusal::of_log(&error))
        }
    }
}

/// 会话目录 `dir` 里主人问过的话，照先后：头一遍放早了的照下界再读一遍，骨架认不得的整份读（同判官）。一整行都还没写完的
/// 是空的（同 `read_events`）。
pub(crate) fn list(dir: &Path, tuning: Tuning) -> Result<Vec<Prompt>, OpenError> {
    let mut sweep = Sweep::new(Plan::guess(), tuning, Asked::default());
    read_range_only(dir, Seq::FIRST, None, ONLY, |events| sweep.feed(events))?;
    let Some(last) = sweep.last else {
        return Ok(Vec::new());
    };
    sweep.finish();
    match sweep.ended {
        Ended::Done => Ok(sweep.settled.done()),
        Ended::Unsure => whole(dir, last),
        Ended::Again => {
            let mut again = Sweep::new(Plan::exact(sweep.reaches), tuning, Asked::default());
            read_range_only(dir, Seq::FIRST, Some(last), ONLY, |events| {
                again.feed(events)
            })?;
            again.finish();
            match again.ended {
                Ended::Done => Ok(again.settled.done()),
                _ => whole(dir, last),
            }
        }
    }
}

/// 整份读到头一遍读到的第 `upto` 条（之后还在往后写的不要），整份交给留着一切的有效历史。
fn whole(dir: &Path, upto: Seq) -> Result<Vec<Prompt>, OpenError> {
    let mut history = History::whole();
    read_range_only(dir, Seq::FIRST, Some(upto), ONLY, |events| {
        for event in events {
            history.append(event);
        }
        Flow::Go
    })?;
    let mut asked = Asked::default();
    asked.settle(history.events());
    Ok(asked.done())
}

/// 收下的问话。
#[derive(Debug, Default)]
struct Asked {
    prompts: Vec<Prompt>,
    /// 闲着时说的、还没见到它开的那一轮的：序号 → 在 `prompts` 里第几句。
    idle: BTreeMap<Seq, usize>,
}

impl Settle for Asked {
    fn settle(&mut self, events: &[Event]) {
        for event in events {
            match &event.body {
                Body::MessageUser(message) if the_persons_own(&event.by) => {
                    if event.turn.is_none() {
                        self.idle.insert(event.seq, self.prompts.len());
                    }
                    self.prompts.push(Prompt {
                        seq: event.seq,
                        turn: event.turn,
                        at: event.at,
                        text: miyu_store::index::said(message, CHARS),
                    });
                }
                Body::TurnStarted(started) => {
                    if let Some(k) = started.trigger.and_then(|seq| self.idle.remove(&seq)) {
                        self.prompts[k].turn = event.turn;
                    }
                }
                _ => {}
            }
        }
    }
}

impl Asked {
    /// 收齐了。
    fn done(self) -> Vec<Prompt> {
        self.prompts
    }
}

/// 主人本人在这里说的：`by` 是主人，带不带 `via` 都算（桥代主人在私聊里转进来的也是）。
fn the_persons_own(by: &By) -> bool {
    matches!(by, By::Person(_))
}

#[cfg(test)]
mod tests;

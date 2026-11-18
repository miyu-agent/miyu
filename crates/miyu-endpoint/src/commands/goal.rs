//! `/goal`（施工 X-8 下，`docs/blueprint/goal.md`「在新的架构里怎么放」第 6 条，`protocol.md` 的 `command.run`）：人交代、看、
//! 停、继续、改、清掉长期目标。照旧版的写法：
//!
//! - `/goal` 看现在的目标；`/goal <目标>` 设一个（当场算授权过，会话闲着就开续轮）；已经有一个没完成的，就是改它（施工 X-8 再补，
//!   2026-10-17 项目主人定：头里翻输入历史调出上一句改了再发，用不着 `edit`），同 `edit`，停着、卡住的改完接着跑；
//! - `/goal pause`、`/goal resume`、`/goal clear`：后面没字的才是子命令，带字的整句当目标；
//! - `/goal edit <目标>` 改原文（照旧认，头不再提）；设、改都能在目标前面写 `--rounds N` 定轮数上限（0 是不限），改的时候不写的照原来的。
//!
//! 卡住的照停着办：人答了 `/goal resume` 接着跑。改目标交内核 `Goal` 命令（照请求的编号），拒的照内核的原因说。

use std::num::NonZeroU32;
use std::sync::Arc;

use miyu_kernel::event::GoalOutcome;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, GoalChange};
use miyu_session::{Goal, GoalState, Handle};

use super::registry::{Call, Commands, Ran, Running, Slash};
use super::{Said, accepted, command, words};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 登记 `/goal`（施工 O-6 三补）。
pub(super) fn register(commands: &mut Commands) {
    commands.register(Arc::new(GoalCommand));
}

/// `/goal`：只在有长期目标的会话里（本机的主会话、长期目标装了、预设开着）。
struct GoalCommand;

impl Slash for GoalCommand {
    fn name(&self) -> &'static str {
        "goal"
    }

    fn package(&self) -> Option<&'static str> {
        Some(super::why::GOAL_PACKAGE)
    }

    fn summary(&self) -> &'static str {
        "commands/summary/goal"
    }

    fn argument(&self) -> Option<&'static str> {
        Some("commands/argument/goal")
    }

    fn usage(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("", "commands/usage/goal/show"),
            ("commands/placeholder/goal", "commands/usage/goal/set"),
            ("pause", "commands/usage/goal/pause"),
            ("resume", "commands/usage/goal/resume"),
            ("clear", "commands/usage/goal/clear"),
        ]
    }

    fn allowed(&self, _core: &Core, handle: &Handle, _by: &By) -> Result<(), Refusal> {
        match handle.goals() {
            true => Ok(()),
            false => Err(Refusal::GOAL_UNAVAILABLE),
        }
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let handle = call.handle();
            let (events, said) = run(
                call.core,
                call.peer,
                (call.session, handle),
                call.id,
                (call.by, call.prefix),
                call.rest,
            )
            .await?;
            Ok(Ran { events, said })
        })
    }
}

/// 人要做的。
#[derive(Debug, PartialEq, Eq)]
enum Asked<'a> {
    Show,
    Set {
        text: &'a str,
        /// 没写 `--rounds` 的没有：新设的不限，改的照原来的（施工 X-8 再补）。
        limit: Option<Option<NonZeroU32>>,
    },
    Edit {
        text: &'a str,
        /// 没写 `--rounds` 的没有：照原来的。
        limit: Option<Option<NonZeroU32>>,
    },
    Pause,
    Resume,
    Clear,
}

/// 照 `asked`（命令后面跟的字，去掉了前后空白）办会话 `handle` 的目标，交回追加的序号和回执。
pub(super) async fn run(
    core: &Core,
    peer: &Peer,
    (session, handle): (&SessionId, &Handle),
    id: &CommandId,
    (by, prefix): (&By, Option<crate::sessions::prefix::Choice>),
    asked: &str,
) -> Result<(Vec<u64>, Said), Refusal> {
    // 停着、卡住的改完接着跑（施工 X-8 再补）。
    let mut resume = false;
    let change = match parse(asked)? {
        Asked::Show => return Ok((Vec::new(), shown(core, peer, session, handle).await?)),
        Asked::Set { text, limit } => match unfinished(core, session, handle).await? {
            Some(goal) => {
                resume = matches!(goal.state, GoalState::Paused | GoalState::Blocked);
                GoalChange::Change {
                    text: text.to_string(),
                    limit: limit.unwrap_or(goal.limit),
                }
            }
            None => GoalChange::Set {
                text: text.to_string(),
                limit: limit.flatten(),
            },
        },
        Asked::Edit { text, limit } => GoalChange::Change {
            text: text.to_string(),
            limit: match limit {
                Some(limit) => limit,
                None => now(core, session, handle)
                    .await?
                    .and_then(|goal| goal.limit),
            },
        },
        Asked::Pause => GoalChange::Pause,
        Asked::Resume => GoalChange::Resume,
        Asked::Clear => GoalChange::End {
            outcome: GoalOutcome::Cleared,
            reason: None,
        },
    };
    // 设定、继续长期目标会开一轮（施工 OV-1 下）：文件改了、前缀要变的先问人。
    if matches!(change, GoalChange::Set { .. } | GoalChange::Resume) {
        crate::sessions::prefix::ask(core, (*peer, by), (handle, id), prefix).await?;
    }
    let said = receipt(&change);
    let mut events =
        accepted(command(core, session, handle, id, by, Command::Goal(change)).await?)?;
    if resume {
        // 另一条命令，编号照这一条接 `/resume`：两条各记各的 `goal.*`，回执只说改了。
        let again = CommandId::parse(&format!("{id}/resume")).map_err(|_| Refusal::BAD_PARAMS)?;
        let resumed = command(
            core,
            session,
            handle,
            &again,
            by,
            Command::Goal(GoalChange::Resume),
        );
        events.extend(accepted(resumed.await?)?);
    }
    Ok((events, said))
}

/// 现在没完成的目标（跑着、等人、停着、卡住）；没有的、完成了的没有。
async fn unfinished(
    core: &Core,
    session: &SessionId,
    handle: &Handle,
) -> Result<Option<Goal>, Refusal> {
    Ok(now(core, session, handle)
        .await?
        .filter(|goal| goal.state != GoalState::Completed))
}

/// 读人写的：见模块的说明。写错的（`edit` 后面没字、`--rounds` 后面不是数、只写了上限）参数不对。
fn parse(asked: &str) -> Result<Asked<'_>, Refusal> {
    if asked.is_empty() {
        return Ok(Asked::Show);
    }
    let (verb, rest) = asked
        .split_once(char::is_whitespace)
        .map_or((asked, ""), |(verb, rest)| (verb, rest.trim()));
    match (verb, rest.is_empty()) {
        ("pause", true) => return Ok(Asked::Pause),
        ("resume", true) => return Ok(Asked::Resume),
        ("clear", true) => return Ok(Asked::Clear),
        ("edit", _) => {
            let (limit, text) = rounds(rest)?;
            return Ok(Asked::Edit { text, limit });
        }
        _ => {}
    }
    let (limit, text) = rounds(asked)?;
    Ok(Asked::Set { text, limit })
}

/// 开头的 `--rounds N`：没写的没有；`0` 是不限。剩下的是目标原文，空的参数不对。
fn rounds(text: &str) -> Result<(Option<Option<NonZeroU32>>, &str), Refusal> {
    let Some(("--rounds", rest)) = text.split_once(char::is_whitespace) else {
        return match text.is_empty() {
            true => Err(Refusal::BAD_PARAMS),
            false => Ok((None, text)),
        };
    };
    let rest = rest.trim_start();
    let (number, text) = rest
        .split_once(char::is_whitespace)
        .map_or((rest, ""), |(number, text)| (number, text.trim()));
    let limit = number.parse::<u32>().map_err(|_| Refusal::BAD_PARAMS)?;
    match text.is_empty() {
        true => Err(Refusal::BAD_PARAMS),
        false => Ok((Some(NonZeroU32::new(limit)), text)),
    }
}

/// 现在的目标：会话的状态里的那一格。会话停了的从表里拿掉。
async fn now(core: &Core, session: &SessionId, handle: &Handle) -> Result<Option<Goal>, Refusal> {
    match handle.current().await {
        Ok(current) => Ok(current.goal),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 改成了的回执：设、改的带原文（换行这些换成空格）。
fn receipt(change: &GoalChange) -> Said {
    let with_text = |key, text: &str| Said {
        key,
        fields: vec![("text", text.replace(char::is_control, " "))],
        lists: Vec::new(),
    };
    match change {
        GoalChange::Set { text, .. } => with_text("commands/goal-set", text),
        GoalChange::Change { text, .. } => with_text("commands/goal-changed", text),
        GoalChange::Pause => Said::plain("commands/goal-paused"),
        GoalChange::Resume => Said::plain("commands/goal-resumed"),
        GoalChange::End { .. } => Said::plain("commands/goal-cleared"),
    }
}

/// `/goal` 看的回执：原文、在哪一步、第几轮（有上限的写共几轮），照连接的语言；没有目标的说没有。
async fn shown(
    core: &Core,
    peer: &Peer,
    session: &SessionId,
    handle: &Handle,
) -> Result<Said, Refusal> {
    let Some(goal) = now(core, session, handle).await? else {
        return Ok(Said::plain("commands/goal-none"));
    };
    let state = match (goal.state, &goal.reason) {
        (GoalState::Blocked, Some(reason)) => Said {
            key: "commands/goal-state/blocked-why",
            fields: vec![("reason", reason.replace(char::is_control, " "))],
            lists: Vec::new(),
        },
        (state, _) => Said::plain(state_key(state)),
    };
    let rounds = match goal.limit {
        Some(limit) => Said {
            key: "commands/goal-rounds-limited",
            fields: vec![
                ("round", goal.round.to_string()),
                ("limit", limit.to_string()),
            ],
            lists: Vec::new(),
        },
        None => Said {
            key: "commands/goal-rounds",
            fields: vec![("round", goal.round.to_string())],
            lists: Vec::new(),
        },
    };
    Ok(Said {
        key: "commands/goal-is",
        fields: vec![
            ("text", goal.text.replace(char::is_control, " ")),
            ("state", words(core, peer, &state).await),
            ("rounds", words(core, peer, &rounds).await),
        ],
        lists: Vec::new(),
    })
}

/// 在哪一步的那个词。
fn state_key(state: GoalState) -> &'static str {
    match state {
        GoalState::Running => "commands/goal-state/running",
        GoalState::Waiting => "commands/goal-state/waiting",
        GoalState::Paused => "commands/goal-state/paused",
        GoalState::Completed => "commands/goal-state/completed",
        GoalState::Blocked => "commands/goal-state/blocked",
    }
}

#[cfg(test)]
mod tests;

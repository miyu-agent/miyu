//! `/todo`（施工 D-3 再补，`docs/blueprint/slash.md`，2026-10-17 项目主人定）：人看、清掉她的待办。
//!
//! - `/todo` 看现在的待办，每项「内容 · 做到哪了」，照语言的分隔连成一行（回执是一行：填进去的字换掉换行）；没有的说没有。
//!   通讯平台上没有侧边栏，也看得到。
//! - `/todo clear` 清掉：交内核 `ClearTodos`，记一条 `todos.cleared`，头照 `todos.changed` 收起清单。没有待办的说没有，什么都
//!   不记（内核拒的 `nothing_to_clear` 也照这样说）。
//!
//! 后面跟别的字的参数不对。跟着 `todowrite` 包（施工 BS-1 下下）：待办是它写的，没装的就没有这条命令。

use std::sync::Arc;

use miyu_config::Words;
use miyu_kernel::event::{Todo, TodoStatus};
use miyu_kernel::session::{Command, Outcome, Reason};
use miyu_store::human::Human;

use super::registry::{Call, Commands, Ran, Running, Slash};
use super::{Said, accepted, command};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 写待办的那个包。
const TODOWRITE: &str = "miyu-base-todowrite";

/// 登记 `/todo`。
pub(super) fn register(commands: &mut Commands) {
    commands.register(Arc::new(TodoCommand));
}

/// `/todo`：谁能用同 `/clear`。
struct TodoCommand;

impl Slash for TodoCommand {
    fn name(&self) -> &'static str {
        "todo"
    }

    /// 跟着 `miyu-base-todowrite` 包（施工 BS-1 下下，设计 44 第三节第 3 条）：待办是它写的，没装的就没有这条。
    fn package(&self) -> Option<&'static str> {
        Some(TODOWRITE)
    }

    fn summary(&self) -> &'static str {
        "commands/summary/todo"
    }

    fn argument(&self) -> Option<&'static str> {
        Some("commands/argument/todo")
    }

    fn usage(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("", "commands/usage/todo/show"),
            ("clear", "commands/usage/todo/clear"),
        ]
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let handle = call.handle();
            match call.rest {
                "" => {
                    let said = shown(call.core, call.peer, &handle.todos()).await;
                    Ok(Ran {
                        events: Vec::new(),
                        said,
                    })
                }
                "clear" => {
                    if handle.todos().is_empty() {
                        return Ok(none());
                    }
                    let clear = Command::ClearTodos;
                    match command(call.core, call.session, handle, call.id, call.by, clear).await? {
                        Outcome::Rejected {
                            reason: Reason::NothingToClear,
                            ..
                        } => Ok(none()),
                        outcome => Ok(Ran {
                            events: accepted(outcome)?,
                            said: Said::plain("commands/todos-cleared"),
                        }),
                    }
                }
                _ => Err(Refusal::BAD_PARAMS),
            }
        })
    }
}

/// 没有待办：什么都不记。
fn none() -> Ran {
    Ran {
        events: Vec::new(),
        said: Said::plain("commands/todo-none"),
    }
}

/// `/todo` 看的回执：每项照连接的语言写好，用那种语言的分隔连起来；没有的说没有。资源读不出来的照没有写的字（空的），同 [`super::words`]。
async fn shown(core: &Core, peer: &Peer, todos: &[Todo]) -> Said {
    if todos.is_empty() {
        return Said::plain("commands/todo-none");
    }
    let resources = core.resources.clone();
    let language = peer.language.to_string();
    let loaded = tokio::task::spawn_blocking(move || Human::load(&resources, &language)).await;
    let Ok(Ok(human)) = loaded else {
        tracing::warn!(target: "miyu::endpoint", "command receipt words not read");
        return Said::plain("commands/todo-none");
    };
    let lines: Vec<String> = todos
        .iter()
        .map(|todo| {
            let state = Words::sentence(&human, state_key(&todo.status), &[]).unwrap_or_default();
            let text = todo.content.replace(char::is_control, " ");
            Words::sentence(
                &human,
                "commands/todo-is",
                &[("text", &text), ("state", &state)],
            )
            .unwrap_or_default()
        })
        .collect();
    let separator = Words::sentence(&human, "commands/todo-separator", &[]).unwrap_or_default();
    Said {
        key: "commands/todo-list",
        fields: vec![("items", lines.join(&separator))],
        lists: Vec::new(),
    }
}

/// 做到哪了的那个词。新版本才有的照还没开始说。
fn state_key(status: &TodoStatus) -> &'static str {
    match status {
        TodoStatus::InProgress => "commands/todo-state/in_progress",
        TodoStatus::Completed => "commands/todo-state/completed",
        _ => "commands/todo-state/pending",
    }
}

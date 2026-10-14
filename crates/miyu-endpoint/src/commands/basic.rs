//! 核心自己的斜杠命令（施工 O-6、9-7 下，O-6 三补起照登记表登记）：`/clear`（别名 `/reset`）清空上下文；`/stop` 全停：打断
//! 这一轮（排着的话留着，不撤回、不接着开），再停掉她派出去的后台命令和子代理；`/workspace` 换会话在哪个目录干活，只有主人
//! 本人能用（`workspace.rs`）。

use std::sync::Arc;

use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Queued, Reason};
use miyu_session::Handle;

use super::registry::{Call, Commands, Ran, Running, Slash};
use super::{Said, accepted, command, workspace};
use crate::Core;
use crate::refusal::Refusal;

/// 登记这三条。
pub(super) fn register(commands: &mut Commands) {
    commands.register(Arc::new(Clear));
    commands.register(Arc::new(Stop));
    commands.register(Arc::new(Workspace));
}

/// `/clear`：同 `session.clear`。
struct Clear;

impl Slash for Clear {
    fn name(&self) -> &'static str {
        "clear"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["reset"]
    }

    fn summary(&self) -> &'static str {
        "commands/summary/clear"
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let cleared = Command::Clear;
            let outcome = command(
                call.core,
                call.session,
                call.handle(),
                call.id,
                call.by,
                cleared,
            )
            .await?;
            Ok(Ran {
                events: accepted(outcome)?,
                said: Said::plain("commands/cleared"),
            })
        })
    }
}

/// `/stop`：打断这一轮，再停掉派出去的。
struct Stop;

impl Slash for Stop {
    fn name(&self) -> &'static str {
        "stop"
    }

    fn summary(&self) -> &'static str {
        "commands/summary/stop"
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let interrupt = Command::Interrupt {
                queued: Queued::Keep,
            };
            let handle = call.handle();
            let mut events = Vec::new();
            match command(call.core, call.session, handle, call.id, call.by, interrupt).await? {
                // 没有在进行的回合：照样往下停后台的。
                Outcome::Rejected {
                    reason: Reason::NotRunning,
                    ..
                } => {}
                outcome => events.extend(accepted(outcome)?),
            }
            if handle
                .stop_jobs(call.by.clone(), call.id.clone())
                .await
                .is_err()
            {
                call.core.sessions.forget(call.session).await;
                return Err(Refusal::STOPPED);
            }
            Ok(Ran {
                events,
                said: Said::plain("commands/stopped"),
            })
        })
    }
}

/// `/workspace <路径>`：只有主人本人能用。
struct Workspace;

impl Slash for Workspace {
    fn name(&self) -> &'static str {
        "workspace"
    }

    fn summary(&self) -> &'static str {
        "commands/summary/workspace"
    }

    fn argument(&self) -> Option<&'static str> {
        Some("commands/argument/workspace")
    }

    /// 动的是沙盒能写的地方：管理的人不行。
    fn allowed(&self, _core: &Core, _handle: &Handle, by: &By) -> Result<(), Refusal> {
        match by.is_owner() {
            true => Ok(()),
            false => Err(Refusal::OWNER_ONLY),
        }
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let (events, said) = workspace::run(
                call.core,
                call.session,
                call.found,
                call.id,
                call.by,
                call.rest,
                call.head,
            )
            .await?;
            Ok(Ran { events, said })
        })
    }
}

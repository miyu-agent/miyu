//! 人用 `!` 打的命令（施工 SH-1 上，`docs/blueprint/session/tools.md`「人跑的命令」）：内核的 `RunShell` 到了，照现在的目录找
//! 声明跑命令的那件工具（`Tool::runs_commands`，预设不管，包关了、没装的没有），调它给人跑的钩子（`Tool::run_for_person`），
//! 不判权限、不进沙盒；跑完的样子从 `backs` 送回，写成内核的 `ShellRan`。`HaltShell` 举起这一条的叫停口子，工具整组杀、
//! 照常交回到这时的输出。一个会话同一时刻最多跑一条（内核一条一条交）。

use std::path::{Path, PathBuf};

use miyu_kernel::event::{ShellRan, ShellReason};
use miyu_kernel::id::Seq;
use miyu_kernel::session::{Action, Input};
use miyu_tool::{Halt, PersonCommand, PersonEnd, PersonRun};

use super::Tools;
use crate::port::Back;

/// 在跑的那一条：哪一句、叫停它的口子。
#[derive(Debug, Default)]
pub(crate) struct Person(Option<(Seq, Halt)>);

impl Tools {
    /// 回内核的 `RunShell`、`HaltShell`：结果另走一路送回，当场不回。
    pub(crate) fn person(&mut self, action: Action) -> Option<Input> {
        match action {
            Action::RunShell {
                message,
                command,
                cwd,
            } => self.run_person(message, command, &cwd),
            Action::HaltShell { message } => self.halt_person(message),
            _ => {}
        }
        None
    }

    /// 跑第 `message` 句里的 `command`，在会话的工作目录 `cwd` 里（`~` 照家目录接上）。没有跑命令的工具、它没有这个钩子、
    /// 起不来的，也送回：输出写为什么，没有退出码。
    fn run_person(&mut self, message: Seq, command: String, cwd: &str) {
        let cwd = miyu_fs::resolve(Path::new("/"), self.home.as_deref(), cwd)
            .unwrap_or_else(|_| PathBuf::from(cwd));
        let catalog = self.catalog.current();
        let tool = catalog
            .specs()
            .filter_map(|spec| catalog.get(&spec.name))
            .find(|tool| tool.runs_commands())
            .cloned();
        let halt = Halt::new();
        self.person.0 = Some((message, halt.clone()));
        let backs = self.backs.clone();
        tokio::spawn(async move {
            let command = PersonCommand { command, cwd };
            let running = tool
                .as_ref()
                .and_then(|tool| tool.run_for_person(command, halt));
            let ran = match running {
                Some(running) => running.await.map_err(|error| error.to_string()),
                None => Err("no tool here runs commands".to_string()),
            };
            #[expect(
                clippy::let_underscore_must_use,
                reason = "actor 停了：这一条跟着会话没了，载入时内核补 aborted"
            )]
            let _ = backs.send(Back::Shell(written(message, ran)));
        });
    }

    /// 叫停第 `message` 句：在跑的是它就举起口子，别的什么都不做。
    fn halt_person(&mut self, message: Seq) {
        if let Some((running, halt)) = &self.person.0
            && *running == message
        {
            halt.halt();
        }
    }

    /// 第 `message` 句跑完了，送回到了：不再是在跑的那一条。
    pub(crate) fn person_done(&mut self, message: Seq) {
        self.person.0.take_if(|(running, _)| *running == message);
    }
}

/// 跑完的样子写成 `shell.ran` 的内容：起不来的，输出是为什么。
fn written(message: Seq, ran: Result<PersonRun, String>) -> ShellRan {
    match ran {
        Ok(ran) => ShellRan {
            message,
            exit: ran.exit,
            signal: ran.signal,
            duration_ms: ran.duration_ms,
            text: ran.text,
            omitted: ran.omitted,
            reason: match ran.ended {
                PersonEnd::Exited => None,
                PersonEnd::TimedOut => Some(ShellReason::TimedOut),
                PersonEnd::Interrupted => Some(ShellReason::Interrupted),
            },
        },
        Err(error) => ShellRan {
            message,
            exit: None,
            signal: None,
            duration_ms: 0,
            text: format!("{error}\n"),
            omitted: 0,
            reason: None,
        },
    }
}

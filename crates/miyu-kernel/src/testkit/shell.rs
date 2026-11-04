//! 替身跑人打的命令（施工 SH-1 上，`session/shell.rs`）：人用 `!` 打一条（[`Stage::shell`]），内核交出 `RunShell` 时照剧本
//! 回（[`Stage::shell_plays`]，没排的照退出码 0、没有输出），扣着的等测试交（[`Stage::finish_shell`]）；叫停的记下来。

use std::collections::VecDeque;

use super::Stage;
use crate::event::{ShellRan, ShellReason};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::session::{Command, Input};

/// 人打的命令接下来怎么回。
#[derive(Debug, Clone)]
pub enum ShellPlay {
    /// 跑完了：退出码、输出。
    Exits(i32, String),
    /// 扣着不回，等测试交（[`Stage::finish_shell`]）。
    Holds,
}

/// 替身这边的几样：交出来跑的（那一句、命令行、工作目录）、剧本、扣着的、叫停过的。
#[derive(Debug, Default)]
pub(super) struct Bench {
    runs: Vec<(Seq, String, String)>,
    plays: VecDeque<ShellPlay>,
    held: Option<Seq>,
    halts: Vec<Seq>,
}

impl Stage {
    /// 人用 `!` 打一条命令。返回这个命令的编号。
    pub fn shell(&mut self, command: &str) -> CommandId {
        self.shell_as(self.by.clone(), command)
    }

    /// 以 `by` 的名义打一条（别的 agent、扩展的，内核拒）。
    pub fn shell_as(&mut self, by: By, command: &str) -> CommandId {
        self.command_as(
            by,
            Command::Shell {
                text: command.to_string(),
                urgent: false,
            },
        )
    }

    /// 不打断，退回一条排着的（`session.withdraw`）：`message` 不写的退回最新的一条。返回这个命令的编号。
    pub fn withdraw(&mut self, message: Option<Seq>) -> CommandId {
        self.command(Command::Withdraw { message })
    }

    /// 人打的命令接下来几次这样回。
    pub fn shell_plays(&mut self, plays: impl IntoIterator<Item = ShellPlay>) {
        self.shells.plays.extend(plays);
    }

    /// 交出来跑的每一条：那一句的序号、命令行、工作目录，照先后。
    pub fn shell_runs(&self) -> &[(Seq, String, String)] {
        &self.shells.runs
    }

    /// 叫停过的，照先后。
    pub fn shell_halts(&self) -> &[Seq] {
        &self.shells.halts
    }

    /// 扣着的那一条交回：退出码或者异常结束的原因、输出。跑到没事可做。
    ///
    /// # Panics
    ///
    /// 没有扣着的。
    pub fn finish_shell(&mut self, exit: Option<i32>, reason: Option<ShellReason>, text: &str) {
        let message = self
            .shells
            .held
            .take()
            .unwrap_or_else(|| panic!("没有扣着的命令"));
        let at = self.tick();
        self.run(Input::ShellRan {
            at,
            ran: ran(message, exit, reason, text),
        });
    }

    /// 内核交出 `RunShell`：记下来，照剧本回。
    pub(super) fn run_shell(&mut self, message: Seq, command: String, cwd: String) -> Vec<Input> {
        self.shells.runs.push((message, command, cwd));
        match self.shells.plays.pop_front() {
            Some(ShellPlay::Holds) => {
                self.shells.held = Some(message);
                Vec::new()
            }
            Some(ShellPlay::Exits(exit, text)) => vec![Input::ShellRan {
                at: self.tick(),
                ran: ran(message, Some(exit), None, &text),
            }],
            None => vec![Input::ShellRan {
                at: self.tick(),
                ran: ran(message, Some(0), None, ""),
            }],
        }
    }

    /// 内核叫停：记下来，扣着的照旧扣着，等测试交。
    pub(super) fn halt_shell(&mut self, message: Seq) -> Vec<Input> {
        self.shells.halts.push(message);
        Vec::new()
    }

    /// 重启、崩了：扣着的跟着没了。
    pub(super) fn drop_shell(&mut self) {
        self.shells.held = None;
    }
}

fn ran(message: Seq, exit: Option<i32>, reason: Option<ShellReason>, text: &str) -> ShellRan {
    ShellRan {
        message,
        exit,
        signal: None,
        duration_ms: 5,
        text: text.to_string(),
        omitted: 0,
        reason,
    }
}

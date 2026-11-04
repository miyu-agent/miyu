//! 给人跑一条命令（施工 SH-1 上，设计 `57-人在会话里跑命令.md` 第二节第 1 条，[`miyu_tool::Tool::run_for_person`]）：**只给人
//! 用**，核心只在收到人发的 `session.send {shell: true}` 时调，她的调用走 `run` 碰不到。
//!
//! 和她的前台调用同一个程序、同一套挑过的环境变量、同一个起法和截法；不同的几样是给她设的挡，人打的不管（2026-11-03 项目主人
//! 当面定）：不套沙盒、不开 `miyu call` 的通道、不查高危（那是权限链看 `Tool::hazard`，这条路不过权限链）、不拦干等的
//! `sleep`、到时（120 秒，同她不写 `timeout` 时的默认）整组杀不挪到后台、不看她看过的文件改没改。人打断时（[`Halt`]）整组杀，
//! 交回到这时的输出。

use std::time::{Duration, Instant};

use miyu_tool::{Exit, Halt, PersonCommand, PersonEnd, PersonRun, PersonRunning, Stop};

use super::output::Shown;
use super::process::{self, Ending, Finished, Guard};
use super::{DEFAULT, Shell, env};
use crate::blocking::blocking;

impl Shell {
    /// 跑人打的 `command`，`halt` 举起来时整组杀，到时照默认的 120 秒。
    pub(super) fn person(&self, command: PersonCommand, halt: Halt) -> PersonRunning<'_> {
        self.person_within(command, halt, Duration::from_millis(DEFAULT))
    }

    /// 同 [`Self::person`]，到时是 `wait`：测试里换短的。
    fn person_within(
        &self,
        command: PersonCommand,
        halt: Halt,
        wait: Duration,
    ) -> PersonRunning<'_> {
        Box::pin(async move {
            let env = env::passed(std::env::vars_os(), false);
            let prepared = self
                .program
                .command(&command.command, &command.cwd, env, None)?;
            let begun = Instant::now();
            let started = process::start(prepared, None, |_| {}, false)?;
            // 起来了就看着：这一次被丢掉时整组杀掉（核心停下、会话没了）。
            let guard = Guard::new(started.group());
            let wakes = process::Wakes::new();
            halt.on_halt(wakes.halter());
            let finished = blocking(Stop::default(), move |_| started.wait(wait, wakes)).await?;
            guard.disarm();
            Ok(self.ran(finished, begun.elapsed()))
        })
    }

    /// 跑完的样子：怎么结束的、退出码或信号、照前台截过的输出和截掉几个字、用时。
    fn ran(&self, finished: Finished, took: Duration) -> PersonRun {
        let Finished { ending, output } = finished;
        let omitted = match output.shown() {
            Shown::Whole(_) => 0,
            Shown::Cut { omitted, .. } => omitted,
        };
        let text = match output.is_empty() {
            true => String::new(),
            false => self.texts.results.body(&output),
        };
        let (ended, exit, signal) = match ending {
            Ending::Exited(status) => match super::background::exit(status) {
                Exit::Code(code) => (PersonEnd::Exited, Some(code), None),
                Exit::Signal(signal) => (PersonEnd::Exited, None, Some(signal)),
            },
            Ending::TimedOut => (PersonEnd::TimedOut, None, None),
            Ending::Halted => (PersonEnd::Interrupted, None, None),
        };
        PersonRun {
            ended,
            exit,
            signal,
            text,
            omitted,
            duration_ms: u64::try_from(took.as_millis()).unwrap_or(u64::MAX),
        }
    }
}

#[cfg(test)]
mod tests;

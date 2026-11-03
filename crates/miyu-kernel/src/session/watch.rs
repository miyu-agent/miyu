//! 监视的后台命令出的行（施工 MN-1 上，设计 56 第三节第 2、4、5 条，`docs/blueprint/kernel/session.md`「回报」）：执行器
//! 在 200 毫秒的窗口里攒好一批交来（输入 `JobLines`），记一条 `job.lines`，照回报的规矩叫醒她（`jobs.rs` 的 `arrive`）。
//!
//! 她这一轮在跑时到的不一批一条：同一条命令的挂在这一轮上，来一批合一次、当场截，到下一步问模型以前合成一条记下、落了盘
//! 再投影（`turn.rs` 的 `advance`）；这一轮没有下一步了的，回合结束时记下，照回报接着开一轮（`queue.rs` 的
//! `finish_turn`）。挂着的只在内存里，核心这时崩了就丢（2026-11-03 主会话照推荐定）。
//!
//! 太吵就停：一条命令最近一小时（照事件的 `at` 滚动算）记满 [`CHATTY`] 条的，再来一批不记，发一次 `StopJobs`（原因
//! `too_chatty`），执行器照停掉的规矩整组杀、报 `job.reported`、叫醒她。数的是账本里记下的条数，合成的算一条。

use super::Session;
use super::action::{Action, StopReason};
use super::input::Input;
use super::policy::Reports;
use crate::event::{Body, JobLines};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

/// 一条 `job.lines` 最多几个字（Unicode 字符，设计 56 第三节第 2 条，2026-11-03 主会话定）：监视本该只打印要她动手的行，
/// 一小时最多 [`CHATTY`] 条，照回报的 3 万字会很快撑满上下文。多了留头尾各一半、中间一行写省了多少（`Reports::cut`）。执行器
/// 攒批时照同一个数边读边截（`miyu-session` 的 `jobs/watch.rs`）。
pub const LINE_CHARS: usize = 4000;

/// 一条监视最近一小时最多记几条 `job.lines`（设计 56 第三节第 5 条）：记满了再来的一批不记，停下报 `too_chatty`。
pub const CHATTY: usize = 120;

/// 挂在这一轮上、还没记下的一批：同一条命令到的几批合在一起，`by`、`cause` 照第一批的（都是起它的那次调用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pending {
    by: By,
    cause: Option<CommandId>,
    lines: JobLines,
}

impl Session {
    /// 执行器交来一批行。读回日志的时候到的先放着；对不上一个还没结束的后台命令的不理；这条命令最近一小时已经记满了的不记，
    /// 头一次碰上发停的动作。她这一轮在跑的挂在这一轮上，闲着的当场记下、照回报看叫不叫醒她。
    pub(super) fn job_lines(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        lines: JobLines,
    ) -> Vec<Action> {
        if let Some(reading) = self.reading.as_mut() {
            reading.later.push(Input::JobLines {
                at,
                by,
                cause,
                lines,
            });
            return Vec::new();
        }
        let job = lines.job.clone();
        if !self.ledger.command_running(&job) {
            return Vec::new();
        }
        if self.ledger.lines_within_hour(&job, at) >= CHATTY {
            return match self.chatty.insert(job.clone()) {
                true => vec![Action::StopJobs {
                    jobs: vec![job],
                    by,
                    cause,
                    reason: StopReason::TooChatty,
                }],
                false => Vec::new(),
            };
        }
        let cut = self.line_cut();
        if let Some(turn) = self.turn.as_mut() {
            match turn
                .lines
                .iter_mut()
                .find(|pending| pending.lines.job == job)
            {
                Some(pending) => merge(&cut, &mut pending.lines, lines),
                None => turn.lines.push(Pending { by, cause, lines }),
            }
            return Vec::new();
        }
        match self.arrive(at, by, cause, Body::JobLines(lines)) {
            Ok(events) => vec![Action::Append(events)],
            Err(_) => Vec::new(),
        }
    }

    /// 把这一轮上挂着的批次记下：下一步问模型以前（`advance`），回合结束的时候（`finish_turn`）。照挂上的先后，一条命令一条，
    /// 照回报排进这一轮的回报队。交回追加的事件；过不了账本的（这时那条命令已经报过结束）不记。
    pub(super) fn record_pending_lines(&mut self, at: Timestamp) -> Vec<crate::event::Event> {
        let pending = self
            .turn
            .as_mut()
            .map(|turn| std::mem::take(&mut turn.lines))
            .unwrap_or_default();
        let mut events = Vec::new();
        for Pending { by, cause, lines } in pending {
            if let Ok(recorded) = self.arrive(at, by, cause, Body::JobLines(lines)) {
                events.extend(recorded);
            }
        }
        events
    }

    /// 一批行的截法：同回报的那一行省略，只是最多 [`LINE_CHARS`] 个字。
    fn line_cut(&self) -> Reports {
        self.policy.reports.for_lines()
    }
}

/// 把新到的一批合进挂着的那一份：行接在后面，再照 [`LINE_CHARS`] 截一次；行数、截掉的字数加起来。
fn merge(cut: &Reports, into: &mut JobLines, more: JobLines) {
    let joined = format!("{}\n{}", into.text, more.text);
    let total = joined.chars().count();
    let (text, shortened) = cut.cut(&joined);
    let dropped = match shortened {
        true => u64::try_from(total.saturating_sub(cut.chars)).unwrap_or(u64::MAX),
        false => 0,
    };
    into.text = text;
    into.lines = into.lines.saturating_add(more.lines);
    into.omitted = into
        .omitted
        .saturating_add(more.omitted)
        .saturating_add(dropped);
}

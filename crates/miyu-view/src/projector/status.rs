//! 会话状态（施工 9-8 补上，`docs/blueprint/view.md`「会话状态」）：喂事件时顺手记下在等什么、正在做什么、上下文用了多少、
//! 最近一次的速度、都在冷却到几时、任务表；[`Projector::status`] 照它们和在跑的这一轮交出整份。

use miyu_kernel::accumulate::Kind;
use miyu_kernel::event::{
    Body as EventBody, CallResult, ChildReason, ChildReported, Effect, ErrorClass, Event,
    JobReason, JobReported, ModelCalled, Piece, Purpose, Transient, TransientBody,
};
use miyu_kernel::id::{CallId, JobId};
use miyu_kernel::time::Timestamp;
use miyu_kernel::waits::Waits;

use super::Projector;
use crate::entry::EntryId;
use crate::status::{
    Doing, FINISHED_KEPT, JobRow, JobState, Preparing, Speed, State, Status, Wait, Waiting,
};

/// 记着的会话状态。
#[derive(Debug, Default)]
pub(super) struct Tracking {
    waiting: Vec<Waiting>,
    doing: Option<Doing>,
    used: Option<u64>,
    speed: Option<Speed>,
    cooling_until: Option<Timestamp>,
    preparing: Option<Preparing>,
    jobs: Vec<JobRow>,
    /// 这一轮等人的那几段（施工 9-8 十四补；9-8 十四补修起用内核的 [`Waits`]，内核记用时也照它，一个口径）。
    waits: Waits,
    /// 脚本里调的工具（施工 M-5 上）：每个父级下面见过的。
    pub(super) nested: super::nested::Seen,
}

impl Projector {
    /// 这一刻投影算得出的会话状态。
    #[must_use]
    pub fn status(&self) -> Status {
        let tracking = &self.tracking;
        let state = match (tracking.waiting.is_empty(), &self.turn) {
            (false, _) => State::Waiting,
            (true, Some(_)) => State::Running,
            (true, None) => State::Idle,
        };
        Status {
            state,
            waiting: tracking.waiting.clone(),
            since: self.turn.as_ref().map(|turn| turn.started),
            // 等完了的那几段：正在等的那一段从 `waiting_since` 起，不算在里面。
            waited_ms: self.turn.as_ref().map_or(0, |turn| {
                let to = tracking.waits.waiting_since().unwrap_or(self.now);
                tracking.waits.waited_ms(turn.started, to)
            }),
            waiting_since: self.turn.as_ref().and(tracking.waits.waiting_since()),
            doing: self.turn.as_ref().and(tracking.doing.clone()),
            used: tracking.used,
            speed: tracking.speed,
            cooling_until: tracking.cooling_until,
            preparing: tracking.preparing,
            jobs: tracking.jobs.clone(),
        }
    }

    /// 一条落了盘的事件喂完以后：记下它改了的状态。
    pub(super) fn track(&mut self, event: &Event) {
        match &event.body {
            EventBody::ApprovalRequested(requested) => {
                let what = (Wait::Approve, requested.nested);
                self.wait(what, requested.call_id, event.at);
            }
            EventBody::QuestionAsked(asked) => {
                self.wait((Wait::Ask, None), asked.call_id, event.at)
            }
            EventBody::ApprovalDecided(decided) => self.settled(decided.call_id, event.at),
            EventBody::QuestionAnswered(answered) => self.settled(answered.call_id, event.at),
            EventBody::ToolResult(result) => {
                self.settled(result.call_id, event.at);
                self.track_effects(event.at, &result.effects);
            }
            EventBody::JobReported(reported) => self.job_ended(event.at, reported),
            EventBody::ChildReported(reported) => self.child_ended(event.at, reported),
            EventBody::ToolNested(nested) => self.nested_settled(nested),
            EventBody::TurnStarted(_) => {
                self.tracking.doing = None;
                self.tracking.waits.clear();
            }
            EventBody::TurnEnded(_) => {
                self.tracking.waiting.clear();
                self.tracking.doing = None;
                self.tracking.waits.clear();
            }
            EventBody::ModelCalled(called) => {
                // 后台提前压的那一次说完了（不论成没成）。
                if called.purpose == Some(Purpose::Compaction)
                    && self
                        .tracking
                        .preparing
                        .is_some_and(|p| p.seen == called.seen.get())
                {
                    self.tracking.preparing = None;
                }
                self.track_called(called);
            }
            EventBody::ContextCompacted(_) => {
                self.tracking.used = None;
                self.tracking.preparing = None;
                if matches!(self.tracking.doing, Some(Doing::Compacting { .. })) {
                    self.tracking.doing = None;
                }
            }
            _ => {}
        }
    }

    /// 一条瞬时的事件喂完以后：正在做什么、压完用了多少、都在冷却到几时。
    pub(super) fn track_transient(&mut self, transient: &Transient) {
        match &transient.body {
            TransientBody::ModelDelta(delta) => {
                if let Piece::Start(kind) = &delta.piece {
                    let entry = EntryId::block(delta.seen, delta.index);
                    self.tracking.doing = Some(match kind {
                        Kind::Text => Doing::Writing { entry },
                        Kind::Reasoning => Doing::Thinking { entry },
                        Kind::ToolCall { .. } => Doing::Tool { entry },
                    });
                }
            }
            TransientBody::Status(status) => {
                let retry = &status.retry;
                let at = later(transient.at, retry.wait_ms);
                if retry.class == ErrorClass::Cooling {
                    self.tracking.cooling_until = Some(at);
                }
                self.tracking.doing = Some(Doing::Retrying {
                    attempt: retry.attempt,
                    limit: retry.limit,
                    at,
                    class: retry.class.clone(),
                    message: retry.message.clone(),
                    status: retry.status,
                    failover: retry.failover,
                });
            }
            TransientBody::CompactionProgress(progress) => {
                self.tracking.doing = Some(Doing::Compacting {
                    entry: EntryId::compaction(progress.seen),
                    trigger: progress.trigger.clone(),
                    written: progress.written,
                    expected: progress.expected,
                });
            }
            TransientBody::CompactionDone(done) => {
                self.tracking.used = Some(done.after);
                self.tracking.doing = None;
                self.tracking.preparing = None;
            }
            TransientBody::NestedStarted(started) => self.nested_started(transient.at, started),
            TransientBody::CompactionStarted(started) => {
                self.tracking.preparing = Some(Preparing {
                    seen: started.seen.get(),
                    since: transient.at,
                });
            }
            _ => {}
        }
    }

    /// 翻页、订阅时切点前的日志：只记任务表（派出、留言叫醒、了结），别的状态照这一页算。
    pub(super) fn learn_rows(&mut self, earlier: &[Event]) {
        for event in earlier {
            match &event.body {
                EventBody::ToolResult(result) => self.track_effects(event.at, &result.effects),
                EventBody::JobReported(reported) => self.job_ended(event.at, reported),
                EventBody::ChildReported(reported) => self.child_ended(event.at, reported),
                // 切点前就开始的后台任务下面调过的（施工 M-5 上）：那一行的个数照整份日志算。
                EventBody::ToolNested(nested) => self.nested_settled(nested),
                _ => {}
            }
        }
    }

    /// 等人的一件：那一步的条目还在才记，`nested` 是脚本里调的那一件的序号（施工 M-7 上）。原来没在等的，起一段等人。
    fn wait(&mut self, (what, nested): (Wait, Option<u32>), call: CallId, at: Timestamp) {
        if let Some(entry) = self.calls.get(&call).cloned()
            && !self.tracking.waiting.iter().any(|w| w.call == call)
        {
            self.tracking.waits.wait(&call, at);
            self.tracking.waiting.push(Waiting {
                what,
                entry,
                call,
                nested,
            });
        }
    }

    /// 那一次调用了结了：不再等人。全了结了，止住这一段等人。
    fn settled(&mut self, call: CallId, at: Timestamp) {
        self.tracking.waiting.retain(|waiting| waiting.call != call);
        self.tracking.waits.settled(&call, at);
    }

    /// `from` 到 `to` 刨掉这一轮等人的那几段，多少毫秒（施工 9-8 十四补，项目主人 2026-10-23 定等人不算用时）：一步、一段、
    /// 一轮的用时都照它算。还在等的那一段算到 `to`。
    pub(super) fn busy_ms(&self, from: Timestamp, to: Timestamp) -> u64 {
        self.tracking.waits.busy_ms(from, to)
    }

    /// 主请求说完了：上下文用了多少、速度；说成了的不再冷却、不再等重试。
    fn track_called(&mut self, called: &ModelCalled) {
        if called.aside() || called.compaction.is_some() {
            return;
        }
        if let Some(usage) = &called.usage {
            // 输入加输出（施工 9-8 十四补）：她回的下一次请求要带上。
            self.tracking.used = Some(usage.input().saturating_add(usage.output));
        }
        if let Some(speed) = Speed::of(called) {
            self.tracking.speed = Some(speed);
        }
        if called.result == CallResult::Ok {
            self.tracking.cooling_until = None;
            if matches!(self.tracking.doing, Some(Doing::Retrying { .. })) {
                self.tracking.doing = None;
            }
        }
    }

    /// 工具结果的效果：派出去的任务记一行，给子代理留了言的叫醒了、回到在跑。
    fn track_effects(&mut self, at: Timestamp, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::JobStarted(started) => {
                    let command = self
                        .jobs
                        .get(&started.job)
                        .and_then(|job| job.command.clone());
                    self.tracking.jobs.retain(|row| row.job != started.job);
                    self.tracking.jobs.push(JobRow {
                        job: started.job.clone(),
                        what: started.what.clone(),
                        title: started.title.clone(),
                        state: JobState::Running,
                        started: at,
                        ended: None,
                        session: started.session.clone(),
                        command,
                        exit_code: None,
                        signal: None,
                        why: None,
                        nested: None,
                        fork: started.fork,
                    });
                }
                Effect::JobMessaged(messaged) => {
                    if let Some(row) = self.row(&messaged.job) {
                        row.state = JobState::Running;
                        row.ended = None;
                        row.why = None;
                    }
                }
                _ => {}
            }
        }
        self.trim_rows();
    }

    /// 后台命令结束了。
    fn job_ended(&mut self, at: Timestamp, reported: &JobReported) {
        let (state, why) = match reported.reason {
            JobReason::Exited if reported.exit_code == Some(0) => (JobState::Done, None),
            JobReason::Exited => (JobState::Failed, None),
            JobReason::Stopped => (JobState::Stopped, Some("stopped")),
            JobReason::Undone => (JobState::Stopped, Some("undone")),
            JobReason::Restarted => (JobState::Stopped, Some("restarted")),
            JobReason::Aborted => (JobState::Aborted, None),
            JobReason::TimedOut => (JobState::Stopped, Some("timed_out")),
            JobReason::TooLarge => (JobState::Stopped, Some("too_large")),
            _ => (JobState::Failed, None),
        };
        if let Some(row) = self.row(&reported.job) {
            row.state = state;
            row.ended = Some(at);
            row.exit_code = reported.exit_code;
            row.signal = reported.signal;
            row.why = why.map(str::to_string);
        }
        self.trim_rows();
    }

    /// 子代理报了。
    fn child_ended(&mut self, at: Timestamp, reported: &ChildReported) {
        let (state, why) = match reported.reason {
            ChildReason::Done => (JobState::Done, None),
            ChildReason::Stopped => (JobState::Stopped, Some("stopped")),
            ChildReason::Undone => (JobState::Stopped, Some("undone")),
            ChildReason::Aborted => (JobState::Aborted, None),
            _ => (JobState::Done, None),
        };
        if let Some(row) = self.row(&reported.job) {
            row.state = state;
            row.ended = Some(at);
            row.why = why.map(str::to_string);
        }
        self.trim_rows();
    }

    pub(super) fn row(&mut self, job: &JobId) -> Option<&mut JobRow> {
        self.tracking.jobs.iter_mut().find(|row| &row.job == job)
    }

    /// 做完的只留最近的几个：照结束的先后，早的先拿掉。
    fn trim_rows(&mut self) {
        let finished = self
            .tracking
            .jobs
            .iter()
            .filter(|row| !row.state.running())
            .count();
        let mut extra = finished.saturating_sub(FINISHED_KEPT);
        while extra > 0 {
            let oldest = self
                .tracking
                .jobs
                .iter()
                .enumerate()
                .filter(|(_, row)| !row.state.running())
                .min_by_key(|(_, row)| row.ended.map(Timestamp::unix_millis))
                .map(|(index, _)| index);
            match oldest {
                Some(index) => {
                    self.tracking.jobs.remove(index);
                    extra -= 1;
                }
                None => break,
            }
        }
    }
}

/// `at` 再过 `ms` 毫秒。
fn later(at: Timestamp, ms: u64) -> Timestamp {
    let ms = i64::try_from(ms).unwrap_or(i64::MAX);
    Timestamp::from_unix_millis(at.unix_millis().saturating_add(ms)).unwrap_or(at)
}

//! 嵌套调用（施工 M-1，设计 `39-脚本调工具.md` 第四节，蓝图 `kernel/session.md`「嵌套调用」）：脚本里的一次 `miyu call`，挂在
//! 前台那次调用或后台任务下面。
//!
//! 一个嵌套调用的一生：来了先查（父级在不在、工具能不能调、参数修不修得了），排进父级的队；轮到了过执行前的链；放行的派去跑，
//! 拒绝的、要问人的当场了结；跑完了结。了结记一条 `tool.nested`，落了盘才答执行器，执行器这时才把结果交给脚本。
//!
//! 不进这一步的调度（[`super::step::Step::ready`]）：父 `shell` 在这一步里独占着，嵌套的排进去会自己等自己。父级要记结果、
//! 报告之前，挂在下面还没了结的都记「已取消」，排在父级那一条前面（[`Session::close_nested`]，由造事件的那几处调）。
//! 只在内存里：崩了以后没有，不补。

use std::collections::{BTreeMap, BTreeSet};

mod types;

pub use types::{NestedAction, NestedAnswer, NestedInput, NestedRefusal};

use super::Session;
use super::action::Action;
use super::input::Verdict;
use super::step::State;
use super::turn::Stage;
use crate::event::{
    Body, Event, NestedStarted, NestedStatus, ToolNested, Transient, TransientBody,
};
use crate::id::{CommandId, ContentHash, NestedId, Parent, Seq};
use crate::origin::By;
use crate::raw::RawJson;
use crate::time::Timestamp;
use crate::tool::{Access, repair};

/// 内存里的嵌套调用。
#[derive(Debug, Default)]
pub(super) struct Nested {
    /// 还没了结的，照到的先后。
    calls: Vec<Call>,
    /// 还活着的父级下面来过的编号：再来的不理。父级结束时清掉。
    seen: BTreeSet<NestedId>,
    /// 这一次输入里父级结束时记下的「已取消」，还没放进追加的那一批（[`Session::splice_nested`]）。
    flushed: Vec<Event>,
    /// 同上，要掐掉的在跑的。
    stops: Vec<NestedId>,
    /// 了结了、记过的：等那一条落了盘答执行器。
    answers: Vec<(Seq, NestedId, NestedAnswer)>,
}

/// 一个还没了结的嵌套调用。
#[derive(Debug)]
struct Call {
    id: NestedId,
    name: String,
    args: String,
    access: Access,
    phase: Phase,
}

/// 走到了哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// 排着。
    Waiting,
    /// 在过链。
    Guarding,
    /// 在跑。
    Running,
}

impl Session {
    /// 脚本里能调的工具：访问类别准、`clicall` 开着的，照名字，带参数格式（施工 M-3）。执行器照它定开不开通道、答
    /// `miyu call --schema`。纯查询。
    pub fn clicall_tools(&self) -> Vec<(String, RawJson)> {
        self.policy
            .tools
            .iter()
            .filter(|(_, rule)| rule.clicall && callable(&rule.access))
            .map(|(name, rule)| (name.clone(), rule.parameters.clone()))
            .collect()
    }

    /// 在跑的嵌套调用，照编号（施工 M-1，`docs/blueprint/protocol.md` 的订阅）：头断了重连时照它接着画 `#序号 显示名`。纯查询。
    pub fn nested_running(&self) -> Vec<NestedStarted> {
        let mut running: Vec<NestedStarted> = self
            .nested
            .calls
            .iter()
            .filter(|call| call.phase == Phase::Running)
            .map(|call| NestedStarted {
                id: call.id.clone(),
                name: call.name.clone(),
            })
            .collect();
        running.sort_by(|a, b| a.id.cmp(&b.id));
        running
    }

    /// 执行器交来的一种回报。
    pub(super) fn nested(&mut self, input: NestedInput) -> Vec<Action> {
        match input {
            NestedInput::Call { at, id, name, args } => self.nested_call(at, id, name, args),
            NestedInput::Guarded { at, id, verdict } => self.nested_guarded(at, &id, verdict),
            NestedInput::Done {
                at,
                id,
                error,
                duration_ms,
                result,
                bytes,
            } => {
                let Some(call) = self.take_nested(&id, Phase::Running) else {
                    return Vec::new();
                };
                let status = match error {
                    true => NestedStatus::Error,
                    false => NestedStatus::Ok,
                };
                let outcome = Outcome {
                    status,
                    duration_ms,
                    result: result.zip(bytes),
                    reason: None,
                };
                self.settle_nested(at, call.id, call.name, call.args, outcome)
            }
        }
    }

    /// 来了一次：照「嵌套调用」第 1 条查，过了的排进父级的队，派不派由 [`Self::dispatch_nested`] 管。
    fn nested_call(
        &mut self,
        at: Timestamp,
        id: NestedId,
        name: String,
        args: String,
    ) -> Vec<Action> {
        if !self.parent_alive(id.parent()) {
            return vec![answer(id, NestedAnswer::Refused(NestedRefusal::Gone))];
        }
        if !self.nested.seen.insert(id.clone()) {
            return Vec::new();
        }
        let checked = match self.policy.tools.get(&name) {
            None => Err(NestedRefusal::Unknown),
            // 两道都过才行（设计 39 第三节第 1 条）：类别是硬规矩，`clicall` 是登记的一方、人说的（施工 M-2）。
            Some(rule) if !callable(&rule.access) || !rule.clicall => {
                Err(NestedRefusal::NotAllowed)
            }
            Some(rule) => repair(&rule.parameters, &args)
                .map(|args| (args, rule.access.clone()))
                .map_err(|_| NestedRefusal::NotAnObject),
        };
        match checked {
            Err(refusal) => vec![answer(id, NestedAnswer::Refused(refusal))],
            // 名单以后会变，只读照样查（设计 39 第四节第 2 条）。
            Ok((args, access)) if access.writes() && self.read_only_now() => {
                let reason = self.policy.tool_texts.read_only().text;
                self.settle_nested(at, id, name, args, Outcome::denied(reason))
            }
            Ok((args, access)) => {
                self.nested.calls.push(Call {
                    id,
                    name,
                    args,
                    access,
                    phase: Phase::Waiting,
                });
                Vec::new()
            }
        }
    }

    /// 链的结论：放行的派去跑，推一条开始了；拒绝的、要问人的当场了结。不是在过链的不理。
    fn nested_guarded(&mut self, at: Timestamp, id: &NestedId, verdict: Verdict) -> Vec<Action> {
        match verdict {
            Verdict::Allow => {
                let (cwd, dirs) = self.place(id.parent());
                let permission = self.effective.clone();
                let turn = self.turn_of(id.parent());
                let Some(call) = self
                    .nested
                    .calls
                    .iter_mut()
                    .find(|call| call.id == *id && call.phase == Phase::Guarding)
                else {
                    return Vec::new();
                };
                call.phase = Phase::Running;
                vec![
                    Action::Nested(NestedAction::Run {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        args: call.args.clone(),
                        cwd,
                        dirs,
                        permission,
                    }),
                    Action::PushTransient(Transient {
                        at,
                        turn: turn.as_ref().map(|(turn, _)| *turn),
                        by: By::Kernel,
                        cause: turn.and_then(|(_, cause)| cause),
                        body: TransientBody::NestedStarted(NestedStarted {
                            id: call.id.clone(),
                            name: call.name.clone(),
                        }),
                    }),
                ]
            }
            Verdict::Deny { text, .. } => match self.take_nested(id, Phase::Guarding) {
                Some(call) => {
                    self.settle_nested(at, call.id, call.name, call.args, Outcome::denied(text))
                }
                None => Vec::new(),
            },
            Verdict::Ask { .. } => match self.take_nested(id, Phase::Guarding) {
                Some(call) => {
                    let outcome = Outcome::kernel(NestedStatus::NeedsApproval);
                    self.settle_nested(at, call.id, call.name, call.args, outcome)
                }
                None => Vec::new(),
            },
        }
    }

    /// 轮到的交给链（「嵌套调用」第 2、3 条）：一个父级一条队，读的一起、别的一个一个；两道上限。每次送进一条输入以后看一次。
    pub(super) fn dispatch_nested(&mut self) -> Vec<Action> {
        let limits = self.policy.nested;
        let busy = |phase: Phase| phase != Phase::Waiting;
        let mut total = self
            .nested
            .calls
            .iter()
            .filter(|call| busy(call.phase))
            .count();
        let mut per_parent: BTreeMap<Parent, usize> = BTreeMap::new();
        for call in self.nested.calls.iter().filter(|call| busy(call.phase)) {
            *per_parent.entry(call.id.parent().clone()).or_default() += 1;
        }
        // 父级照编号，同一个父级里照到的先后。
        let mut order: Vec<usize> = (0..self.nested.calls.len()).collect();
        order.sort_by(|&a, &b| {
            self.nested.calls[a]
                .id
                .parent()
                .cmp(self.nested.calls[b].id.parent())
        });
        let mut ready = Vec::new();
        let mut earlier: BTreeMap<Parent, (bool, bool)> = BTreeMap::new();
        for k in order {
            let call = &self.nested.calls[k];
            let parent = call.id.parent().clone();
            let (pending, exclusive) = earlier.get(&parent).copied().unwrap_or_default();
            let mine = call.access != Access::Read;
            let free = if mine { !pending } else { !exclusive };
            let room = per_parent.get(&parent).copied().unwrap_or(0) < limits.per_parent
                && total < limits.per_session;
            if call.phase == Phase::Waiting && free && room {
                ready.push(k);
                total += 1;
                *per_parent.entry(parent.clone()).or_default() += 1;
            }
            earlier.insert(parent, (true, exclusive || mine));
        }
        let permission = self.effective.clone();
        let grants = self.grants.granted();
        let mut actions = Vec::new();
        for k in ready {
            let (cwd, dirs) = self.place(self.nested.calls[k].id.parent());
            let call = &mut self.nested.calls[k];
            call.phase = Phase::Guarding;
            actions.push(Action::Nested(NestedAction::Guard {
                id: call.id.clone(),
                name: call.name.clone(),
                args: call.args.clone(),
                cwd,
                dirs,
                permission: permission.clone(),
                grants: grants.clone(),
            }));
        }
        actions
    }

    /// 要记 `body` 了：它是父级的结果、报告的，挂在下面还没了结的先记「已取消」，在跑的记下要掐（「嵌套调用」第 7 条）。造
    /// 事件的那几处在给 `body` 编序号之前调；记下的放在一边，送完这条输入时放进同一批（[`Self::splice_nested`]）。
    pub(super) fn close_nested(&mut self, at: Timestamp, body: &Body) {
        let parent = match body {
            Body::ToolResult(result) => Parent::Call(result.call_id),
            Body::JobReported(reported) => Parent::Job(reported.job.clone()),
            _ => return,
        };
        let (closing, kept): (Vec<Call>, Vec<Call>) = std::mem::take(&mut self.nested.calls)
            .into_iter()
            .partition(|call| *call.id.parent() == parent);
        self.nested.calls = kept;
        self.nested.seen.retain(|id| *id.parent() != parent);
        for call in closing {
            if call.phase == Phase::Running {
                self.nested.stops.push(call.id.clone());
            }
            let outcome = Outcome::kernel(NestedStatus::Cancelled);
            let event = self.nested_event(at, call.id, call.name, call.args, outcome);
            self.nested.flushed.push(event);
        }
    }

    /// 送完一条输入：父级结束时记下的「已取消」放进追加的那一批，排在父级那一条前面；要掐的接在最后。
    ///
    /// # Panics
    ///
    /// 记下的「已取消」后面那一条不在这一次的动作里：造事件的一处没把父级那一条交出去，是内核的 bug。
    pub(super) fn splice_nested(&mut self, actions: &mut Vec<Action>) {
        let mut flushed = std::mem::take(&mut self.nested.flushed);
        flushed.sort_by_key(|event| event.seq);
        // 从后往前插：每一条插在序号紧接着它的那一条前面，那一条也可能是刚插进去的「已取消」。
        for event in flushed.into_iter().rev() {
            let after = event.seq.next();
            let placed = actions.iter_mut().find_map(|action| match action {
                Action::Append(events) => events
                    .iter()
                    .position(|later| later.seq == after)
                    .map(|k| (events, k)),
                _ => None,
            });
            let Some((events, k)) = placed else {
                panic!(
                    "nested call {} was cancelled but its parent's event is not appended, a kernel bug",
                    event.seq
                );
            };
            events.insert(k, event);
        }
        actions.extend(
            std::mem::take(&mut self.nested.stops)
                .into_iter()
                .map(|id| Action::Nested(NestedAction::Cancel { id })),
        );
    }

    /// 落到了 `upto`：了结的那一条落了盘的，答执行器。
    pub(super) fn answer_nested(&mut self, upto: Seq) -> Vec<Action> {
        let (landed, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.nested.answers)
            .into_iter()
            .partition(|(seq, _, _)| *seq <= upto);
        self.nested.answers = waiting;
        landed
            .into_iter()
            .map(|(_, id, settled)| answer(id, settled))
            .collect()
    }

    /// 了结一个：记一条，交回追加的动作。
    fn settle_nested(
        &mut self,
        at: Timestamp,
        id: NestedId,
        name: String,
        args: String,
        outcome: Outcome,
    ) -> Vec<Action> {
        vec![Action::Append(vec![
            self.nested_event(at, id, name, args, outcome),
        ])]
    }

    /// 造一条 `tool.nested`，记下落了盘要答的。前台的带这一轮、`cause` 是回合的；后台的不带回合、没有 `cause`。
    fn nested_event(
        &mut self,
        at: Timestamp,
        id: NestedId,
        name: String,
        args: String,
        outcome: Outcome,
    ) -> Event {
        let (result, bytes) = outcome
            .result
            .map_or((None, None), |(hash, n)| (Some(hash), Some(n)));
        let settled = NestedAnswer::Settled {
            status: outcome.status.clone(),
            reason: outcome.reason.clone(),
        };
        let body = Body::ToolNested(ToolNested {
            id: id.clone(),
            name,
            args,
            status: outcome.status,
            duration_ms: outcome.duration_ms,
            result,
            bytes,
            reason: outcome.reason,
        });
        let event = match self.turn_of(id.parent()) {
            Some((_, cause)) => self.record(at, By::Kernel, cause, body),
            None => self.record_outside(at, By::Kernel, None, body),
        };
        self.nested.answers.push((event.seq, id, settled));
        event
    }

    /// 拿走 `id`，要它正在 `phase`；不是的不动、交回没有。
    fn take_nested(&mut self, id: &NestedId, phase: Phase) -> Option<Call> {
        let k = self
            .nested
            .calls
            .iter()
            .position(|call| call.id == *id && call.phase == phase)?;
        Some(self.nested.calls.remove(k))
    }

    /// 父级还活着：前台的是这一步里在跑的调用（打断着、停着的不算），后台的是还没报过结束的后台命令。
    fn parent_alive(&self, parent: &Parent) -> bool {
        match parent {
            Parent::Call(call_id) => self.turn.as_ref().is_some_and(|turn| {
                turn.interrupting.is_none()
                    && matches!(&turn.stage, Stage::Tools(step) if step.calls.iter().any(|call| {
                        call.id == *call_id
                            && matches!(call.state, State::Running | State::Questioning | State::Answered { .. })
                    }))
            }),
            Parent::Job(job) => self.ledger.command_running(job),
        }
    }

    /// 前台的：这一轮的编号和 `cause`。后台的没有。
    fn turn_of(&self, parent: &Parent) -> Option<(crate::id::TurnId, Option<CommandId>)> {
        match parent {
            Parent::Call(_) => self.turn.as_ref().map(|turn| (turn.id, turn.cause.clone())),
            Parent::Job(_) => None,
        }
    }

    /// 工作目录和加进来的目录：前台的照这一轮的，后台的照会话现在的环境。
    fn place(&self, parent: &Parent) -> (String, Vec<String>) {
        match (parent, self.turn.as_ref()) {
            (Parent::Call(_), Some(turn)) => (turn.cwd.clone(), turn.dirs.clone()),
            _ => (self.environment.cwd.clone(), self.environment.dirs.clone()),
        }
    }
}

/// 了结成什么样。
struct Outcome {
    status: NestedStatus,
    duration_ms: Option<u64>,
    result: Option<(ContentHash, u64)>,
    reason: Option<String>,
}

impl Outcome {
    /// 内核了结的：没执行过。
    fn kernel(status: NestedStatus) -> Outcome {
        Outcome {
            status,
            duration_ms: None,
            result: None,
            reason: None,
        }
    }

    /// 拒绝的，带那一句。
    fn denied(reason: String) -> Outcome {
        Outcome {
            reason: Some(reason),
            ..Outcome::kernel(NestedStatus::Denied)
        }
    }
}

/// 第一版的嵌套调用只准这几类（设计 39 MC2）。
fn callable(access: &Access) -> bool {
    matches!(access, Access::Read | Access::Network | Access::Execute)
}

/// 答脚本那头的动作。
fn answer(id: NestedId, answer: NestedAnswer) -> Action {
    Action::Nested(NestedAction::Answer { id, answer })
}

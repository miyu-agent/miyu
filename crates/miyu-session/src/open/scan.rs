//! 从日志里认出来的几样（施工 3-9 六补从 `load.rs` 挪来：那一份到了行数上限）：现在的快照、最近一次记下的工作目录；载入时
//! 边读边记的那几样（施工 V-15，[`Scan`]），派子代理的账（施工 V-37 上，[`Dispatched`]）。

use std::collections::BTreeSet;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{CallId, ContentHash, SessionId};
use miyu_kernel::origin::Model;
use miyu_tool::is_subagent;

/// 现在的快照（施工 P-1 再补）：整份日志里最近一条带 `policy` 的 `session.policy_changed`，撤掉的回合里的也算（换快照不是
/// 对话的一部分）；没换过的没有，照 `session.created` 的。
pub(crate) fn current_policy(events: &[Event]) -> Option<&ContentHash> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::PolicyChanged(changed) => changed.policy.as_ref(),
        _ => None,
    })
}

/// 日志里最后一次记下的工作目录、加进来的目录（施工 4-9 再补三上、5-10 上、9-7 上；V-2 三补从会话表挪来）：工作目录照列会话的
/// 同一个认法（`miyu_store::index::cwd`：最后一条带 `cwd` 的 `turn.started`、换工作区，没有就照 `session.created`）；加进来的
/// 目录照最后一条 `turn.started` 的，换工作区写了的照它，都没有的是空的。一路记的是 [`Place`]，这一份整份找的留给测试对照。
#[cfg(test)]
pub(crate) fn remembered(events: &[Event]) -> (Option<String>, Vec<String>) {
    let cwd = events
        .iter()
        .rev()
        .find_map(|event| miyu_store::index::cwd(event).map(str::to_string));
    let dirs = events
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::TurnStarted(started) => Some(started.dirs.clone()),
            Body::WorkspaceChanged(changed) => changed.dirs.clone(),
            _ => None,
        })
        .unwrap_or_default();
    (cwd, dirs)
}

/// 最后一次记下的工作目录、加进来的目录（同测试里整份找的 `remembered`），照先后一条条记（施工 V-39 中：分叉时边读边记，载入时 [`Scan`]
/// 也照它记）。
#[derive(Debug, Default)]
pub(crate) struct Place {
    /// 最后一次记下的工作目录。
    cwd: Option<String>,
    /// 最后一次记下的加进来的目录。
    dirs: Option<Vec<String>>,
}

impl Place {
    /// 记一条。
    pub(crate) fn note(&mut self, event: &Event) {
        if let Some(cwd) = miyu_store::index::cwd(event) {
            self.cwd = Some(cwd.to_string());
        }
        match &event.body {
            Body::TurnStarted(started) => self.dirs = Some(started.dirs.clone()),
            Body::WorkspaceChanged(changed) => {
                if let Some(dirs) = &changed.dirs {
                    self.dirs = Some(dirs.clone());
                }
            }
            _ => {}
        }
    }

    /// 最后一次记下的工作目录、加进来的目录。
    pub(crate) fn remembered(&self) -> (Option<String>, Vec<String>) {
        (self.cwd.clone(), self.dirs.clone().unwrap_or_default())
    }
}

/// 载入时边读边记的几样（施工 V-15：日志一块块读，读过的放掉，从整份里找的改成一路记）：每一样同上面整份找的那个。
#[derive(Debug, Default)]
pub(super) struct Scan {
    /// 最近一条带 `policy` 的 `session.policy_changed` 的快照（同 [`current_policy`]）。
    pub(super) policy: Option<ContentHash>,
    /// 换了人格的 `session.policy_changed`，照先后：记忆认换到这个人格的那一条（`memory::recast::floor`）只看这几条。
    pub(super) recasts: Vec<Event>,
    /// 最后一条带着她在场所里叫什么、带着群名的人话（施工 E-1 修三补、E-4 上）：记忆载入时照它们接上最近听到的（`Heard::last`）。
    heard: [Option<Event>; 2],
    /// 最后一次记下的工作目录、加进来的目录（同测试里整份找的 `remembered`）。
    place: Place,
    /// 她看过的文件（`effects::seen_in`）。
    pub(super) seen: crate::effects::SeenScan,
    /// 派出去的任务（`Roster::from_events`）。
    pub(super) roster: crate::jobs::Roster,
    /// 最近一条发出去了的 `model.called` 发给了谁（施工 8-8）：钉住的池载入时照它认钉着的成员（「起草时定的」第 2 条）。
    pub(super) sent: Option<Model>,
    /// 累计的用量和计数（`Tally::of`）。
    pub(super) tally: crate::current::Tally,
    /// 长期目标（`Goals::of`）。
    pub(super) goals: crate::goal::Goals,
    /// 派子代理的账（施工 V-37 上）。
    pub(super) dispatched: Dispatched,
    /// 最后一条的时刻。
    pub(super) last: Option<miyu_kernel::time::Timestamp>,
    /// 读了几条。
    pub(super) count: usize,
}

impl Scan {
    /// 先记分叉出来的底子（父会话到分叉点为止的）：她看过的文件照底子、自己的先后算（同以前把两份接起来），别的几样只看
    /// 自己的日志。
    pub(super) fn base(&mut self, base: &[Event]) {
        for event in base {
            self.seen.note(event);
        }
    }

    /// 同 [`Scan::base`]，分叉时已经照底子一条条记好的 `seen`（施工 V-39 下）：要在记自己的以前交。
    pub(super) fn based(&mut self, seen: crate::effects::SeenScan) {
        self.seen = seen;
    }

    /// 读完了，要照哪一份快照：`now` 是照着取了策略的那一份；最近一次换的（`policy`）和它不一样的交回换成的，一样的、没换过的
    /// （照 `created`）没有。
    pub(super) fn switched(&self, now: &ContentHash, created: &ContentHash) -> Option<ContentHash> {
        let last = self.policy.as_ref().unwrap_or(created);
        (last != now).then(|| last.clone())
    }

    /// 记下照先后的一块。
    pub(super) fn note(&mut self, events: &[Event]) {
        for event in events {
            self.place.note(event);
            match &event.body {
                Body::PolicyChanged(changed) => {
                    if let Some(policy) = &changed.policy {
                        self.policy = Some(policy.clone());
                    }
                    if changed.persona.is_some() {
                        self.recasts.push(event.clone());
                    }
                }
                Body::MessageUser(message) => {
                    if let Some(venue) = &message.venue {
                        for (n, has) in [venue.my_name.is_some(), venue.place.is_some()]
                            .into_iter()
                            .enumerate()
                        {
                            if has {
                                self.heard[n] = Some(event.clone());
                            }
                        }
                    }
                }
                Body::ModelCalled(called) => {
                    if let (Some(endpoint), Some(model)) = (&called.endpoint, &called.model) {
                        self.sent = Some(Model {
                            endpoint: endpoint.clone(),
                            model: model.clone(),
                        });
                    }
                }
                _ => {}
            }
            self.seen.note(event);
            self.roster.note(event);
            self.dispatched.note(event);
        }
        self.tally.add(events);
        self.goals.add(events);
        self.last = events.last().map(|event| event.at).or(self.last);
        self.count += events.len();
    }

    /// 接上记忆时交的那几条（`memory::Laid::Read`）：换了人格的，加最后一条带着她叫什么、带着群名的人话，照先后。
    pub(super) fn laid(&self) -> Vec<Event> {
        let mut laid = self.recasts.clone();
        for event in self.heard.iter().flatten() {
            if !laid.iter().any(|seen| seen.seq == event.seq) {
                laid.push(event.clone());
            }
        }
        laid.sort_by_key(|event| event.seq);
        laid
    }

    /// 最后一次记下的工作目录、加进来的目录（同测试里整份找的 `remembered`）。
    pub(super) fn remembered(&self) -> (Option<String>, Vec<String>) {
        self.place.remembered()
    }
}

/// 载入交回的（施工 V-2 三补、V-37 上）：会话这时在哪干活，要不要认它派到一半的空子会话。
#[derive(Debug)]
pub struct Placed {
    /// 在哪干活：照日志里最后一次记下的挑的，会话表照它记，不用再问 actor（施工 V-2 三补）。
    pub environment: Environment,
    /// 要不要认它派到一半的空子会话（施工 V-37 上，`agents.md` 第一条第 8 条）：日志里派子代理的调用，结果里都记下了
    /// `job.started` 的不用认，是 `None`；有没记下的，是记下了的那些子会话，会话表里 `parent` 是它、又不在这里面的才是空
    /// 子会话。照载入读完日志的那一刻。
    pub recorded: Option<BTreeSet<SessionId>>,
}

/// 派子代理的账（施工 V-37 上）：派出去的 `subagent` 调用（以前造的会话里叫 `agent`，也算），结果里带着子会话的
/// `job.started` 的调用和那些子会话。原来收空子会话时另把整份日志读一遍攒这三样，现在载入时一条条记；只看自己的日志，
/// 分叉出来的底子不算（同原来只读自己的目录）。
#[derive(Debug, Default)]
pub(super) struct Dispatched {
    /// 派子代理的调用。
    calls: BTreeSet<CallId>,
    /// 结果里带着子会话的 `job.started` 的调用。
    started_calls: BTreeSet<CallId>,
    /// 那些 `job.started` 里的子会话。
    started: BTreeSet<SessionId>,
}

impl Dispatched {
    /// 记下一条。
    fn note(&mut self, event: &Event) {
        match &event.body {
            Body::MessageAssistant(reply) => {
                self.calls
                    .extend(reply.blocks.iter().filter_map(|block| match block {
                        Block::ToolCall(call) if is_subagent(&call.name) => Some(call.call_id),
                        _ => None,
                    }));
            }
            Body::ToolResult(result) => {
                for effect in &result.effects {
                    if let Effect::JobStarted(job) = effect
                        && let Some(session) = &job.session
                    {
                        self.started_calls.insert(result.call_id);
                        self.started.insert(session.clone());
                    }
                }
            }
            _ => {}
        }
    }

    /// 读完了，连同在哪干活 `environment` 交回。
    pub(super) fn placed(self, environment: Environment) -> Placed {
        Placed {
            environment,
            recorded: self.recorded(),
        }
    }

    /// 要不要认（[`Placed::recorded`]）：派子代理的调用都在带着子会话的 `job.started` 的调用里的不用认；不然交回记下了的
    /// 子会话。
    fn recorded(self) -> Option<BTreeSet<SessionId>> {
        (!self.calls.is_subset(&self.started_calls)).then_some(self.started)
    }
}

#[cfg(test)]
mod tests;

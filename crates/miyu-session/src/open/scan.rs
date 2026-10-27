//! 从日志里认出来的几样（施工 3-9 六补从 `load.rs` 挪来：那一份到了行数上限）：现在的快照、最近一次记下的工作目录；载入时
//! 边读边记的那几样（施工 V-15，[`Scan`]）。

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::ContentHash;
use miyu_kernel::origin::Model;

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
/// 目录照最后一条 `turn.started` 的，换工作区写了的照它，都没有的是空的。
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

/// 载入时边读边记的几样（施工 V-15：日志一块块读，读过的放掉，从整份里找的改成一路记）：每一样同上面整份找的那个。
#[derive(Debug, Default)]
pub(super) struct Scan {
    /// 最近一条带 `policy` 的 `session.policy_changed` 的快照（同 [`current_policy`]）。
    pub(super) policy: Option<ContentHash>,
    /// 换了人格的 `session.policy_changed`，照先后：记忆认换到这个人格的那一条（`memory::recast::floor`）只看这几条。
    pub(super) recasts: Vec<Event>,
    /// 最后一条带着她在场所里叫什么的人话（施工 E-1 修三补）：记忆载入时照它接上她最近在场所里叫什么（`her::last_called`）。
    called: Option<Event>,
    /// 最后一次记下的工作目录（同 [`remembered`]）。
    cwd: Option<String>,
    /// 最后一次记下的加进来的目录（同 [`remembered`]）。
    dirs: Option<Vec<String>>,
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

    /// 读完了，要照哪一份快照：`now` 是照着取了策略的那一份；最近一次换的（`policy`）和它不一样的交回换成的，一样的、没换过的
    /// （照 `created`）没有。
    pub(super) fn switched(&self, now: &ContentHash, created: &ContentHash) -> Option<ContentHash> {
        let last = self.policy.as_ref().unwrap_or(created);
        (last != now).then(|| last.clone())
    }

    /// 记下照先后的一块。
    pub(super) fn note(&mut self, events: &[Event]) {
        for event in events {
            if let Some(cwd) = miyu_store::index::cwd(event) {
                self.cwd = Some(cwd.to_string());
            }
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
                    if message
                        .venue
                        .as_ref()
                        .is_some_and(|venue| venue.my_name.is_some())
                    {
                        self.called = Some(event.clone());
                    }
                }
                Body::TurnStarted(started) => self.dirs = Some(started.dirs.clone()),
                Body::WorkspaceChanged(changed) => {
                    if let Some(dirs) = &changed.dirs {
                        self.dirs = Some(dirs.clone());
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
        }
        self.tally.add(events);
        self.goals.add(events);
        self.last = events.last().map(|event| event.at).or(self.last);
        self.count += events.len();
    }

    /// 接上记忆时交的那几条（`memory::Laid::Read`）：换了人格的，加最后一条带着她叫什么的人话，照先后。
    pub(super) fn laid(&self) -> Vec<Event> {
        let mut laid = self.recasts.clone();
        laid.extend(self.called.clone());
        laid.sort_by_key(|event| event.seq);
        laid
    }

    /// 最后一次记下的工作目录、加进来的目录（同 [`remembered`]）。
    pub(super) fn remembered(&self) -> (Option<String>, Vec<String>) {
        (self.cwd.clone(), self.dirs.clone().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests;

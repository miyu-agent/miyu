//! 看守查嵌套调用（施工 M-1，`docs/blueprint/kernel/session.md`「嵌套调用」）：
//!
//! - 两条不变式：父调用的 `tool.result`、任务的 `job.reported` 之前，挂在下面收下了的都已了结；
//! - 每个收下的只了结一次；前台的带回合，后台的不带；`clicall` 没开的当场拒、一条都不记（施工 M-2）；
//! - 交给链的是收下了、还排着的，一个父级、整个会话同时过链和在跑的不超过上限；派去跑的是链刚放行的；掐掉的是在跑的；
//! - 推给头的开始了是在跑的；答脚本那头的「了结了」都记过、落了盘。
//! - 要问人的（施工 M-7 上，`kernel/asking.md`「脚本里调的要人确认」）：请求只在链说要问人时记，写的是链交回的，前台、有人能
//!   确认的才记；一个父级同时只问一件，有一件在问的父级不再交给链；在问的占名额；人允许了才派；`needs_approval` 只在没人能确认、
//!   挂在后台任务下面时记。

use super::*;
use crate::event::{NestedStarted, NestedStatus};
use crate::id::{NestedId, Parent};
use crate::session::{NestedAction, NestedAnswer, NestedInput, NestedLimits, NestedRefusal};

/// 看守手里的嵌套调用。
#[derive(Debug, Default)]
pub(in super::super) struct Nesting {
    /// 收下了、还没了结的。
    pub(in super::super) queued: BTreeSet<NestedId>,
    /// 其中在过链的。
    pub(in super::super) guarding: BTreeSet<NestedId>,
    /// 其中在跑的。
    pub(in super::super) running: BTreeSet<NestedId>,
    /// 其中在等人确认的（施工 M-7 上）：照日志里的请求。
    pub(in super::super) asking: BTreeSet<NestedId>,
    /// 链说要问人的，和那个结论：请求要写它交回的。
    asked: BTreeMap<NestedId, crate::session::Verdict>,
    /// 了结过的（还活着的父级下面的）：再记一次就错了。
    settled: BTreeSet<NestedId>,
    /// 记过、还没答脚本那头的：记在第几条。
    unanswered: BTreeMap<NestedId, Seq>,
    /// 每个父级下面编到第几号。
    pub(in super::super) next: BTreeMap<Parent, u32>,
    /// 这一次输入里刚了结、原来在跑的：等着它的掐掉。
    cut: BTreeSet<NestedId>,
    /// 这一次输入是链放行了哪一个。
    allowed: Option<NestedId>,
}

/// 送进去的是嵌套调用的回报的，留一份：送进去以后照内核的回答认（[`Watch::nested_fed`]）。
pub(super) fn of(input: &Input) -> Option<NestedInput> {
    match input {
        Input::Nested(nested) => Some(nested.clone()),
        _ => None,
    }
}

/// 随机测试里脚本不能调的那一件（施工 M-2，`random_policy`）：类别准（读），`clicall` 关。
pub(in super::super) const CLICALL_OFF: &str = "ask_user";

/// 随机测试的上限：照 [`policy`] 的。
const LIMITS: NestedLimits = NestedLimits {
    per_parent: 8,
    per_session: 16,
};

impl Watch {
    /// 送进去之前、之后：收没收下照内核的回答认；链放行了哪一个记下，派去跑的要对得上。
    pub(super) fn nested_fed(&mut self, nested: Option<NestedInput>, actions: &[Action]) {
        self.nesting.allowed = None;
        let Some(nested) = nested else {
            return;
        };
        match &nested {
            NestedInput::Call { id, name, .. } => {
                let refused = actions.iter().find_map(|action| match action {
                    Action::Nested(NestedAction::Answer {
                        id: answered,
                        answer: NestedAnswer::Refused(refusal),
                    }) if answered == id => Some(*refusal),
                    _ => None,
                });
                let known = self.nesting.queued.contains(id) || self.nesting.settled.contains(id);
                // 同一个编号再来的内核不理、什么都不答（`known`），只查头一回来的。
                if name == CLICALL_OFF && !known {
                    assert!(
                        matches!(
                            refused,
                            Some(NestedRefusal::NotAllowed | NestedRefusal::Gone)
                        ),
                        "种子 {}：{id} 调的 {name} 没开 clicall，应该当场拒：{actions:?}",
                        self.seed
                    );
                    if refused == Some(NestedRefusal::NotAllowed) {
                        self.seen_paths.insert("clicall 没开的当场拒");
                    }
                }
                match refused {
                    // 父级不在了的不算来过；用法不对的来过了，再来内核不理。
                    Some(refusal) => {
                        self.seen_paths.insert("嵌套调用没收下");
                        if refusal != NestedRefusal::Gone {
                            self.nesting.settled.insert(id.clone());
                        }
                    }
                    None if known => {}
                    None => {
                        self.nesting.queued.insert(id.clone());
                    }
                }
            }
            NestedInput::Guarded { id, verdict, .. }
                if self.nesting.guarding.contains(id)
                    && *verdict == crate::session::Verdict::Allow =>
            {
                self.nesting.allowed = Some(id.clone());
            }
            // 要问人的：当场了结、记请求、退回排队三选一，哪一种都不再在过链。
            NestedInput::Guarded { id, verdict, .. }
                if self.nesting.guarding.contains(id)
                    && matches!(verdict, crate::session::Verdict::Ask { .. }) =>
            {
                self.nesting.guarding.remove(id);
                self.nesting.asked.insert(id.clone(), verdict.clone());
            }
            _ => {}
        }
    }

    /// 追加的一条：嵌套调用的了结、父级的结束。
    pub(super) fn nested_event(&mut self, event: &Event) {
        let seed = self.seed;
        match &event.body {
            Body::ToolNested(nested) => {
                let id = &nested.id;
                assert_ne!(
                    nested.name, CLICALL_OFF,
                    "种子 {seed}：{id} 调的工具没开 clicall，却记了一条"
                );
                assert!(
                    self.nesting.queued.remove(id),
                    "种子 {seed}：{id} 没收下、或者了结过了，又记了一条"
                );
                let was_running = self.nesting.running.remove(id);
                self.nesting.guarding.remove(id);
                if self.nesting.asking.remove(id) {
                    self.seen_paths.insert("在问人的嵌套调用了结了");
                }
                if nested.status == NestedStatus::NeedsApproval {
                    assert!(
                        !self.approvals.attended || matches!(id.parent(), Parent::Job(_)),
                        "种子 {seed}：{id} 前台、有人能确认，应该停下来问"
                    );
                }
                assert!(self.nesting.settled.insert(id.clone()));
                self.nesting.unanswered.insert(id.clone(), event.seq);
                if was_running && nested.status == NestedStatus::Cancelled {
                    self.nesting.cut.insert(id.clone());
                }
                if matches!(nested.status, NestedStatus::Ok | NestedStatus::Error) {
                    assert!(was_running, "种子 {seed}：{id} 没在跑，却跑完了");
                    self.seen_paths.insert("嵌套调用跑完了");
                }
                match id.parent() {
                    Parent::Call(_) => assert!(
                        event.turn.is_some(),
                        "种子 {seed}：{id} 挂在调用下面，要带回合"
                    ),
                    Parent::Job(_) => assert!(
                        event.turn.is_none(),
                        "种子 {seed}：{id} 挂在任务下面，不带回合"
                    ),
                }
            }
            Body::ToolResult(result) => self.parent_ends(&Parent::Call(result.call_id)),
            Body::JobReported(reported) => self.parent_ends(&Parent::Job(reported.job.clone())),
            _ => {}
        }
    }

    /// 父级结束了：挂在下面收下的都已了结（不变式），了结过的清掉。
    fn parent_ends(&mut self, parent: &Parent) {
        let seed = self.seed;
        if let Some(left) = self.nesting.queued.iter().find(|id| id.parent() == parent) {
            panic!("种子 {seed}：{parent} 结束了，{left} 还没了结");
        }
        let before = self.nesting.settled.len();
        self.nesting.settled.retain(|id| id.parent() != parent);
        if self.nesting.settled.len() < before {
            self.seen_paths.insert("父级结束时下面有嵌套调用");
        }
    }

    /// 嵌套调用的动作。
    pub(super) fn nested_action(&mut self, action: NestedAction) {
        let seed = self.seed;
        match action {
            NestedAction::Guard { id, .. } => {
                assert!(
                    self.nesting.queued.contains(&id)
                        && !self.nesting.guarding.contains(&id)
                        && !self.nesting.running.contains(&id)
                        && !self.nesting.asking.contains(&id),
                    "种子 {seed}：{id} 不是排着的，却交给了链"
                );
                assert!(
                    !self
                        .nesting
                        .asking
                        .iter()
                        .any(|other| other.parent() == id.parent()),
                    "种子 {seed}：{} 下面有一件在问人，{id} 却交给了链",
                    id.parent()
                );
                self.nesting.guarding.insert(id.clone());
                let busy: Vec<&NestedId> = self
                    .nesting
                    .guarding
                    .iter()
                    .chain(&self.nesting.running)
                    .chain(&self.nesting.asking)
                    .collect();
                let mine = busy
                    .iter()
                    .filter(|other| other.parent() == id.parent())
                    .count();
                assert!(
                    mine <= LIMITS.per_parent,
                    "种子 {seed}：{} 下面同时过链、在跑的超了上限",
                    id.parent()
                );
                assert!(
                    busy.len() <= LIMITS.per_session,
                    "种子 {seed}：会话里同时过链、在跑的超了上限"
                );
            }
            NestedAction::Run { id, .. } => {
                assert_eq!(
                    self.nesting.allowed.take().as_ref(),
                    Some(&id),
                    "种子 {seed}：{id} 不是链刚放行的、人刚允许的，却派去跑了"
                );
                assert!(self.nesting.guarding.remove(&id) || self.nesting.asking.remove(&id));
                self.nesting.running.insert(id);
            }
            NestedAction::Cancel { id } => {
                assert!(
                    self.nesting.cut.remove(&id),
                    "种子 {seed}：{id} 不在跑，却叫掐掉"
                );
                self.seen_paths.insert("掐掉了在跑的嵌套调用");
            }
            NestedAction::Answer {
                id,
                answer: NestedAnswer::Settled { .. },
            } => {
                let seq = self.nesting.unanswered.remove(&id);
                let seq = seq.unwrap_or_else(|| panic!("种子 {seed}：{id} 没记过，却答了了结"));
                assert!(
                    self.pushed.contains(&seq),
                    "种子 {seed}：{id} 记的那一条还没落盘就答了"
                );
            }
            NestedAction::Answer { .. } => {}
        }
    }

    /// 推给头的开始了：是在跑的。
    pub(super) fn nested_started(&mut self, started: &NestedStarted) {
        let seed = self.seed;
        assert!(
            self.nesting.running.contains(&started.id),
            "种子 {seed}：{} 不在跑，却推了开始",
            started.id
        );
    }

    /// 载入了一个新会话：在路上的都跟着没了（崩了不补），记过的照旧不能再记。
    pub(super) fn nested_reloaded(&mut self) {
        self.nesting.queued.clear();
        self.nesting.guarding.clear();
        self.nesting.running.clear();
        self.nesting.asking.clear();
        self.nesting.asked.clear();
        self.nesting.cut.clear();
        self.nesting.unanswered.clear();
    }

    /// 脚本里调的那一件的请求（施工 M-7 上）：链说过要问人、写的是它交回的，前台、有人能确认，只读时不是要写入的，这个父级
    /// 没有别的在问。
    pub(super) fn nested_requested(
        &mut self,
        event: &Event,
        request: &crate::event::ApprovalRequested,
        n: u32,
    ) {
        let seed = self.seed;
        let id = NestedId::new(Parent::Call(request.call_id), n).unwrap();
        let Some(crate::session::Verdict::Ask {
            module,
            access,
            rule,
            detail,
        }) = self.nesting.asked.get(&id)
        else {
            panic!("种子 {seed}：链没说 {id} 要问人，却记了请求");
        };
        assert_eq!(
            event.by,
            By::Module(crate::origin::Module { id: module.clone() })
        );
        assert_eq!(
            (&request.access, &request.rule, &request.detail),
            (access, rule, detail),
            "种子 {seed}：{id} 的请求写的不是链交回的"
        );
        assert!(
            self.approvals.attended,
            "种子 {seed}：没人能确认，{id} 不该问"
        );
        assert!(
            !(access.writes() && self.read_only_in_effect()),
            "种子 {seed}：只读时要写入的 {id}，不该问"
        );
        assert!(
            !self
                .nesting
                .asking
                .iter()
                .any(|other| other.parent() == id.parent()),
            "种子 {seed}：{} 下面同时问了两件",
            id.parent()
        );
        assert!(
            self.nesting.queued.contains(&id),
            "种子 {seed}：{id} 没收下却问了人"
        );
        self.seen_paths.insert("脚本里调的停下来问人");
        self.nesting.asking.insert(id);
    }

    /// 脚本里调的那一件的决定：在问的才有；允许的接着就派。
    pub(super) fn nested_decided(
        &mut self,
        event: &Event,
        decided: &crate::event::ApprovalDecided,
        n: u32,
    ) {
        let seed = self.seed;
        let id = NestedId::new(Parent::Call(decided.call_id), n).unwrap();
        assert!(
            self.nesting.asking.contains(&id),
            "种子 {seed}：{id} 不在等人，却有了决定"
        );
        assert!(
            matches!(event.by, By::Person(_)),
            "种子 {seed}：决定是人做的"
        );
        if decided.decision == crate::event::Decision::Deny {
            self.seen_paths.insert("人拒绝了脚本里调的");
        } else {
            self.seen_paths.insert("人允许了脚本里调的");
            self.nesting.allowed = Some(id);
        }
    }

    /// 回答脚本里调的：`call_id` 下面正在问的那一件（写了序号的要对上）和它的请求提没提规则。
    pub(super) fn nested_asking(&self, call_id: CallId, n: Option<u32>) -> Option<bool> {
        let id =
            self.nesting.asking.iter().find(|id| {
                *id.parent() == Parent::Call(call_id) && n.is_none_or(|n| id.n() == n)
            })?;
        Some(matches!(
            self.nesting.asked.get(id),
            Some(crate::session::Verdict::Ask { rule: Some(_), .. })
        ))
    }

    /// 还在跑的后台命令（看守记着的任务里，不是子代理、没结束的）。
    pub(in super::super) fn running_commands(&self) -> Vec<crate::id::JobId> {
        self.reports
            .jobs
            .iter()
            .filter(|(_, job)| job.session.is_none() && !job.over)
            .map(|(job, _)| job.clone())
            .collect()
    }
}

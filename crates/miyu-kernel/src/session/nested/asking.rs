//! 脚本里调的要人确认（施工 M-7 上，设计 `39-脚本调工具.md` MC4，蓝图 `kernel/asking.md`「脚本里调的要人确认」）：前台的、
//! 有人能确认的主会话里停下来问，同直接调用一样答；别的照旧当场了结成 `needs_approval`，脚本拿到退出码 4。
//!
//! 请求记在父级那次调用上，带嵌套的序号。一个父级同时只问一件：不带序号的回答就认得出答的是哪一件，现在的头照 `call_id` 把
//! 卡片挂在 `shell` 那一步上、答的时候不带序号，不改也答得上；账本、放行过的规则、等人的计时照调用编号认，也不用改。

use super::super::action::{Action, Reason};
use super::super::approval::Reply;
use super::super::{Session, rejected};
use super::{Outcome, Phase};
use crate::event::{ApprovalDecided, ApprovalRequested, Body, NestedStatus};
use crate::id::{CallId, CommandId, ModuleId, NestedId, Parent};
use crate::origin::{By, Module};
use crate::raw::RawJson;
use crate::time::Timestamp;
use crate::tool::Access;

impl Session {
    /// 挂在 `parent` 下面的要问人时停不停下来问：父级是前台那次调用，有人能确认，是主会话（`session.created` 没有 `parent`；
    /// 子会话的 `attended` 照父会话的，只看它挡不住子代理）。
    fn asks_from_scripts(&self, parent: &Parent) -> bool {
        matches!(parent, Parent::Call(_)) && self.policy.attended && !self.duty.child()
    }

    /// 链说 `id` 要问人：`by` 是 `module`，要的是 `access`，提的规则 `rule`，给头看的 `detail`。
    ///
    /// - 不停下来问的（[`Self::asks_from_scripts`]）当场了结成 `needs_approval`；只读时要写入的了结成 `denied`。
    /// - 同一个父级已经有一件在问的，退回排队：那件答了、了结了再过链，那时「本会话都允许」可能已经放行了它。
    /// - 别的记一条请求，`call_id` 是父级那次调用、`nested` 是序号，`cause` 是回合的；这件停下来等人，不派、不推开始。
    ///
    /// 不是在过链的不理。
    pub(super) fn nested_ask(
        &mut self,
        at: Timestamp,
        id: &NestedId,
        (module, access): (ModuleId, Access),
        rule: Option<RawJson>,
        detail: Option<RawJson>,
    ) -> Vec<Action> {
        let parent = id.parent().clone();
        let call_id = match &parent {
            Parent::Call(call_id) if self.asks_from_scripts(&parent) => *call_id,
            _ => return self.not_asked(at, id, Outcome::kernel(NestedStatus::NeedsApproval)),
        };
        if self.read_only_now() && access.writes() {
            let reason = self.policy.tool_texts.read_only().text;
            return self.not_asked(at, id, Outcome::denied(reason));
        }
        let busy = self
            .nested
            .calls
            .iter()
            .any(|call| call.phase == Phase::Asking && *call.id.parent() == parent);
        let Some(call) = self
            .nested
            .calls
            .iter_mut()
            .find(|call| call.id == *id && call.phase == Phase::Guarding)
        else {
            return Vec::new();
        };
        if busy {
            call.phase = Phase::Waiting;
            return Vec::new();
        }
        call.phase = Phase::Asking;
        call.rule = rule.is_some();
        let request = ApprovalRequested {
            call_id,
            access,
            rule,
            detail,
            nested: Some(id.n()),
        };
        let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
        let by = By::Module(Module { id: module });
        let event = self.record(at, by, cause, Body::ApprovalRequested(request));
        vec![Action::Append(vec![event])]
    }

    /// 不问人，把过着链的 `id` 了结成 `outcome`。
    fn not_asked(&mut self, at: Timestamp, id: &NestedId, outcome: Outcome) -> Vec<Action> {
        match self.take_nested(id, Phase::Guarding) {
            Some(call) => self.settle_nested(at, call.id, call.name, call.args, outcome),
            None => Vec::new(),
        }
    }

    /// 人回答了 `call_id` 下面脚本里调的那一件（`nested` 写了的只认那一件，没写的认正在问的那一件）：记一条决定，`by` 是
    /// 回答的人，`cause` 是这个命令。允许的派去跑；拒绝的了结成 `denied`，那一句照直接调用（`denied`、`denied-with-reason`），
    /// 交给脚本；附件照记在决定里，脚本收不到。回应附上这一次追加的事件的序号。
    ///
    /// 没有在问的、序号对不上的 `not_asking`；选项、规则、理由照直接调用查（[`Reply::judged`]）。
    pub(in crate::session) fn decide_nested(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        (call_id, nested): (CallId, Option<u32>),
        reply: Reply,
    ) -> Vec<Action> {
        let parent = Parent::Call(call_id);
        let Some(k) = self.nested.calls.iter().position(|call| {
            call.phase == Phase::Asking
                && *call.id.parent() == parent
                && nested.is_none_or(|n| call.id.n() == n)
        }) else {
            return vec![rejected(id, Reason::NotAsking)];
        };
        let deny = match reply.judged(self.nested.calls[k].rule) {
            Ok(deny) => deny,
            Err(reason) => return vec![rejected(id, reason)],
        };
        let asked = self.nested.calls[k].id.clone();
        let Reply {
            decision,
            reason,
            attachments,
        } = reply;
        let body = ApprovalDecided {
            call_id,
            decision,
            reason: reason.clone(),
            attachments,
            nested: Some(asked.n()),
        };
        let mut events = vec![self.record(at, by, Some(id.clone()), Body::ApprovalDecided(body))];
        let mut then = Vec::new();
        if deny {
            let call = self.nested.calls.remove(k);
            let text = self.policy.tool_texts.denied(reason.as_deref()).text;
            events.push(self.nested_event(
                at,
                call.id,
                call.name,
                call.args,
                Outcome::denied(text),
            ));
        } else {
            then = self.launch(at, &asked, Phase::Asking);
        }
        self.accept(id, events.iter().map(|event| event.seq).collect());
        let mut actions = vec![Action::Append(events)];
        actions.extend(then);
        actions
    }
}

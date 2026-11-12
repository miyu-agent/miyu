//! 确认（`docs/designs/02-内核.md` 第六节「确认怎么走」）：执行前的链交回的结论，人的回答。
//!
//! 链放行的派；拒绝的当场记下；要问人的记一条请求，这个调用停下来等人。人允许的，决定落了盘
//! 再派；拒绝的，记一条被拒绝的结果，这一步照常往下走，她接着干（`11-权限与沙盒.md` A13）。
//! 这一轮配了等多久的，请求带上截止的时刻，到时还没人确认，内核替人拒绝（施工 CD-1）。

use super::action::{Action, Reason};
use super::input::{Answer, Input, Verdict};
use super::retry::later;
use super::step::{Asked, State};
use super::tools::run_tool;
use super::turn::Stage;
use super::{Session, rejected};
use crate::block::Block;
use crate::event::{ApprovalDecided, ApprovalRequested, Body, Decision, Event, ToolStatus};
use crate::id::{CallId, CommandId};
use crate::origin::{By, Module};
use crate::time::Timestamp;
use crate::tool::Worded;

/// 链的结论落到一个调用上以后，要记什么。
enum Settled {
    /// 被拒绝了：谁拒的，写给模型的那一句和给人看的说法。
    Denied(By, Worded),
    /// 要问人：谁问的，请求。
    Asked(By, ApprovalRequested),
}

impl Session {
    /// 执行前的链判完了。放行的派；拒绝的记一条被拒绝的结果，`by` 是拒绝的模块；要问人的，记一条
    /// 请求，`by` 是提问的模块，这个调用停下来等人。没人能确认的、只读时要的是写入的，不记请求，
    /// 当场拒绝，`by` 是内核。不是这一步在等结论的调用，不理：被打断、跳过、拦下以后迟到的就是这种。
    pub(super) fn tool_guarded(
        &mut self,
        at: Timestamp,
        call_id: CallId,
        verdict: Verdict,
    ) -> Vec<Action> {
        let read_only = self.read_only_now();
        let attended = self.policy.attended;
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let cause = turn.cause.clone();
        let round = turn.round();
        // 等确认的截止时刻（施工 CD-1）：记请求的这一刻加这一轮的等多久；一直等的没有。
        let expires_at = turn.confirm_ms.map(|ms| later(at, ms));
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let Some(call) = step.find(call_id, State::Guarding) else {
            return Vec::new();
        };
        let texts = &self.policy.tool_texts;
        let settled = match verdict {
            Verdict::Allow => {
                call.state = State::Running;
                return vec![run_tool(
                    call_id,
                    &call.name,
                    &call.args,
                    &round,
                    &self.effective,
                )];
            }
            Verdict::Deny {
                module,
                text,
                human,
            } => Settled::Denied(
                By::Module(Module { id: module }),
                Worded { text, said: human },
            ),
            Verdict::Ask { .. } if !attended => Settled::Denied(By::Kernel, texts.unattended()),
            Verdict::Ask { access, .. } if read_only && access.writes() => {
                Settled::Denied(By::Kernel, texts.read_only())
            }
            Verdict::Ask {
                module,
                access,
                rule,
                detail,
            } => {
                call.asked = Some(Asked {
                    access: access.clone(),
                    rule: rule.is_some(),
                });
                let request = ApprovalRequested {
                    call_id,
                    access,
                    rule,
                    detail,
                    nested: None,
                    expires_at,
                };
                Settled::Asked(By::Module(Module { id: module }), request)
            }
        };
        call.state = match settled {
            Settled::Denied(..) => State::Done,
            Settled::Asked(..) => State::Asking,
        };
        let finished = step.finished();
        let expiry = match settled {
            Settled::Asked(..) => expires_at.map(|at| Action::ExpireApproval {
                at,
                call_id,
                nested: None,
            }),
            Settled::Denied(..) => None,
        };
        let mut events = match settled {
            Settled::Denied(by, worded) => {
                vec![self.written_result(
                    at,
                    by,
                    cause.clone(),
                    call_id,
                    ToolStatus::Denied,
                    worded,
                )]
            }
            Settled::Asked(by, request) => {
                vec![self.record(at, by, cause.clone(), Body::ApprovalRequested(request))]
            }
        };
        if finished {
            events.extend(self.finish_step(at, cause));
        }
        let mut actions = vec![Action::Append(events)];
        actions.extend(expiry);
        actions.extend(self.dispatch());
        actions
    }

    /// 等确认等到了截止的时刻（施工 CD-1，`kernel/asking.md`「到时没人确认」）：这件还在等确认的，内核替人拒绝，记一条拒绝的
    /// 决定、一条被拒绝的结果，`by` 都是内核，`cause` 是回合的，那一句和说法是执行器交的；这一步照常往下走，她看到结果换别的办法。
    /// 脚本里调的照 [`Session::expire_nested`]。不在等的不理：人先定了、有了结果、作废了、被打断了。
    pub(super) fn approval_expired(&mut self, expired: Input) -> Vec<Action> {
        let Input::ApprovalExpired {
            at,
            call_id,
            nested,
            text,
            human,
        } = expired
        else {
            return Vec::new();
        };
        let worded = Worded { text, said: human };
        if let Some(n) = nested {
            return self.expire_nested(at, (call_id, n), worded);
        }
        if self.asking(call_id).is_none() {
            return Vec::new();
        }
        let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
        let body = ApprovalDecided {
            call_id,
            decision: Decision::Deny,
            reason: None,
            attachments: Vec::new(),
            nested: None,
        };
        let decided = self.record(at, By::Kernel, cause.clone(), Body::ApprovalDecided(body));
        let mut events = vec![decided];
        events.extend(self.denied_by(at, (By::Kernel, cause), call_id, (worded, Vec::new())));
        let mut actions = vec![Action::Append(events)];
        actions.extend(self.dispatch());
        actions
    }

    /// 人回答了一次确认：记一条决定，`by` 是回答的人，`cause` 是这个命令。允许的，决定落了盘
    /// 再派；拒绝的，再记一条被拒绝的结果，写了理由的带上理由，这一步照常往下走。回应附上这一次
    /// 追加的全部事件的序号。
    ///
    /// 不在等确认的、选项不认识的、请求没提规则却选了记住的、允许却带了理由、附件的，拒绝这个命令。
    /// 空的理由当没写。拒绝时附的图、文件接在理由后面进结果（施工 Q-A）。
    pub(super) fn answer(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        call_id: CallId,
        answer: Answer,
    ) -> Vec<Action> {
        let Answer::Approval {
            decision,
            reason,
            attachments,
            nested,
        } = answer
        else {
            return vec![rejected(id, Reason::NotAsking)];
        };
        let reply = Reply {
            decision,
            reason: reason.filter(|reason| !reason.trim().is_empty()),
            attachments,
        };
        // 不是这次调用自己在等确认的，看它下面脚本里调的那一件（施工 M-7 上）：写了序号的只看那一件。
        let Some(rule) = self.asking(call_id).filter(|_| nested.is_none()) else {
            return self.decide_nested(id, by, at, (call_id, nested), reply);
        };
        let deny = match reply.judged(rule) {
            Ok(deny) => deny,
            Err(reason) => return vec![rejected(id, reason)],
        };
        let Reply {
            decision,
            reason,
            attachments,
        } = reply;
        let body = ApprovalDecided {
            call_id,
            decision,
            reason: reason.clone(),
            attachments: attachments.clone(),
            nested: None,
        };
        let decided = self.record(
            at,
            by.clone(),
            Some(id.clone()),
            Body::ApprovalDecided(body),
        );
        let seq = decided.seq;
        let mut events = vec![decided];
        if deny {
            let text = self.policy.tool_texts.denied(reason.as_deref());
            let cause = Some(id.clone());
            events.extend(self.denied_by(at, (by, cause), call_id, (text, attachments)));
        } else {
            self.set_state(call_id, State::Approved { decided: seq });
        }
        self.accept(id, events.iter().map(|event| event.seq).collect());
        let mut actions = vec![Action::Append(events)];
        actions.extend(self.dispatch());
        actions
    }

    /// 拒绝了：这个调用有了结果，记一条被拒绝的结果，写的是 `worded`，附件接在后面。人拒的 `by` 是拒绝的人、`cause` 是回答的
    /// 命令；到时没人确认的（施工 CD-1）`by` 是内核、`cause` 是回合的。这一步因此齐了的，往下走。
    fn denied_by(
        &mut self,
        at: Timestamp,
        (by, cause): (By, Option<CommandId>),
        call_id: CallId,
        (worded, attachments): (Worded, Vec<Block>),
    ) -> Vec<Event> {
        let finished = self.set_state(call_id, State::Done);
        let mut event = self.written_result(at, by, cause, call_id, ToolStatus::Denied, worded);
        if let Body::ToolResult(result) = &mut event.body {
            result.blocks.extend(attachments);
        }
        let mut events = vec![event];
        if finished {
            let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
            events.extend(self.finish_step(at, cause));
        }
        events
    }

    /// 这一步里在等人回答的 `call_id`：它的请求提没提放行规则。不在等的，没有。
    fn asking(&self, call_id: CallId) -> Option<bool> {
        let Stage::Tools(step) = &self.turn.as_ref()?.stage else {
            return None;
        };
        step.calls
            .iter()
            .find(|call| call.id == call_id && call.state == State::Asking)
            .map(|call| call.asked.as_ref().is_some_and(|asked| asked.rule))
    }

    /// 把这一步里的 `call_id` 换成 `state`，返回这一步齐了没有。
    fn set_state(&mut self, call_id: CallId, state: State) -> bool {
        let Some(Stage::Tools(step)) = self.turn.as_mut().map(|turn| &mut turn.stage) else {
            return false;
        };
        if let Some(call) = step.calls.iter_mut().find(|call| call.id == call_id) {
            call.state = state;
        }
        step.finished()
    }
}

/// 人对一次确认的回答：选项，去掉空白不是空的理由，附件。
pub(super) struct Reply {
    pub(super) decision: Decision,
    pub(super) reason: Option<String>,
    pub(super) attachments: Vec<Block>,
}

impl Reply {
    /// 照请求提没提放行规则 `rule` 查：交回是不是拒绝。选项不认识的、没提规则却选了记住的、允许却带了理由、附件的，交回
    /// 不收的原因。直接调用、脚本里调的一个查法（施工 M-7 上）。
    pub(super) fn judged(&self, rule: bool) -> Result<bool, Reason> {
        match self.decision {
            Decision::Other(_) => Err(Reason::UnknownDecision),
            Decision::Session | Decision::Workspace if !rule => Err(Reason::NoRule),
            Decision::Deny => Ok(true),
            _ if self.reason.is_some() || !self.attachments.is_empty() => {
                Err(Reason::UnexpectedReason)
            }
            _ => Ok(false),
        }
    }
}

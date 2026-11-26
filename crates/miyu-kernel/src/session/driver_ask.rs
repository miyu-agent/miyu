//! 驱动在一次请求进行中问人（施工 ACP-8，设计 73 第四节第 7 条，蓝图 `kernel/asking.md`「驱动问的确认」）：扩展驱动交过来的
//! agent 要跑命令、改文件，照工具调用的确认那一套问人，头照旧画、照旧答。
//!
//! - 问得了的：在路上的正是那一次主请求、那一块是这一次已经开始了的供应商侧工具、有人能确认、不是只读时要写要跑命令的。
//!   记一条 `tool.approval_requested`，`by` 是那个包，`driver` 写画在哪一块，`call_id` 是内核另给的 `call_<这一条的序号>_1`
//!   （不对着调用）；配了等多久的带上截止的时刻。问不了的不记，当场回（[`DriverAnswer`]）。
//! - 人答了（`session.answer`）：照工具调用的确认查（没有规则，只能允许这一次或者拒绝），记一条决定，落了盘再回驱动。
//! - 到时没人确认：内核替人拒，记一条决定，`by` 是内核，落了盘再回。
//! - 这一次请求有了结局（说完了、出错、被打断、撤掉重发、重启）：还在等的各记一条拒绝的决定，`by` 是内核，当场回「没了」。

use super::Session;
use super::action::{Action, Reason, rejected};
use super::approval::Reply;
use super::input::Answer;
use super::retry::later;
use super::turn::Stage;
use crate::accumulate::Kind;
use crate::event::{ApprovalDecided, ApprovalRequested, Body, Decision, DriverBlock, Event};
use crate::id::{CallId, CommandId, ModuleId, Seq};
use crate::origin::{By, Module};
use crate::raw::RawJson;
use crate::time::Timestamp;
use crate::tool::Access;

/// 驱动要人确认（[`super::Input::DriverAsked`]）：执行器读好了驱动交的，写成给头看的 `detail`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverAsked {
    /// 到的时刻，取自执行器的时钟。
    pub at: Timestamp,
    /// 哪一次请求。
    pub seen: Seq,
    /// 执行器给这一问的编号：回的时候照它找问的那一头（[`Action::AnswerDriver`]）。
    pub token: u64,
    /// 驱动所在的包：记成请求的 `by`。
    pub module: ModuleId,
    /// 要的是哪一类访问。
    pub access: Access,
    /// 画在这一次的第几块供应商侧工具上。
    pub index: u32,
    /// 给头看的，原样记。
    pub detail: RawJson,
}

/// 回驱动的话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverAnswer {
    /// 人允许这一次。
    Allowed,
    /// 人拒绝了，写了理由的带上。
    Denied {
        /// 去掉空白不是空的理由。
        reason: Option<String>,
    },
    /// 没问人就拒了。
    Auto(AutoDeny),
    /// 这一次请求没了（说完了、被打断、撤掉重发），或者问的不是在路上的那一次。
    Cancelled,
    /// 那一块不是这一次已经开始了的供应商侧工具。
    NotHosted,
}

/// 没问人就拒的缘故。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoDeny {
    /// 没人能确认：场所会话、没有头接着开的会话。
    Unattended,
    /// 只读时要写、要跑命令（对方跑的不在 Miyu 的沙盒里）。
    ReadOnly,
    /// 到时没人确认。
    Expired,
    /// 压缩的摘要请求、辅助请求（回顾、起标题这类）里问的。
    Aside,
}

impl AutoDeny {
    /// 协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            AutoDeny::Unattended => "unattended",
            AutoDeny::ReadOnly => "read_only",
            AutoDeny::Expired => "expired",
            AutoDeny::Aside => "aside",
        }
    }
}

/// 在等人的一问：执行器给的编号，内核给的调用编号。
#[derive(Debug)]
pub(super) struct Asking {
    token: u64,
    call_id: CallId,
}

impl Session {
    /// 驱动要人确认：见模块的说明。
    pub(super) fn driver_asked(&mut self, asked: DriverAsked) -> Vec<Action> {
        let DriverAsked {
            at,
            seen,
            token,
            module,
            access,
            index,
            detail,
        } = asked;
        let answer = |answer| vec![Action::AnswerDriver { token, answer }];
        let read_only = self.read_only_now() && (access.writes() || access == Access::Execute);
        let attended = self.policy.attended;
        let Some(turn) = self.turn.as_ref() else {
            return answer(DriverAnswer::Cancelled);
        };
        let (cause, expires_at) = (turn.cause.clone(), turn.confirm_ms.map(|ms| later(at, ms)));
        let Stage::Asking(call) = &turn.stage else {
            return answer(DriverAnswer::Cancelled);
        };
        let hosted = usize::try_from(index)
            .ok()
            .and_then(|index| call.accumulator.kind(index));
        if call.seen != seen {
            return answer(DriverAnswer::Cancelled);
        }
        if !matches!(hosted, Some(Kind::HostedTool { .. })) {
            return answer(DriverAnswer::NotHosted);
        }
        let auto = if call.compaction.is_some() {
            Some(AutoDeny::Aside)
        } else if !attended {
            Some(AutoDeny::Unattended)
        } else {
            read_only.then_some(AutoDeny::ReadOnly)
        };
        if let Some(auto) = auto {
            return answer(DriverAnswer::Auto(auto));
        }
        let call_id = CallId::new(self.ledger.next_seq(), 1).expect("第几个从 1 数起");
        if let Some(call) = self.call(seen) {
            call.asks.push(Asking { token, call_id });
        }
        let request = ApprovalRequested {
            call_id,
            access,
            rule: None,
            detail: Some(detail),
            nested: None,
            expires_at,
            driver: Some(DriverBlock { seen, index }),
        };
        let by = By::Module(Module { id: module });
        let event = self.record(at, by, cause, Body::ApprovalRequested(Box::new(request)));
        let mut actions = vec![Action::Append(vec![event])];
        actions.extend(expires_at.map(|at| Action::ExpireApproval {
            at,
            call_id,
            nested: None,
        }));
        actions
    }

    /// `call_id` 是驱动问的、还在等人的一问。
    pub(super) fn driver_asking(&self, call_id: CallId) -> bool {
        match self.turn.as_ref().map(|turn| &turn.stage) {
            Some(Stage::Asking(call)) => call.asks.iter().any(|ask| ask.call_id == call_id),
            _ => false,
        }
    }

    /// 人回答了驱动问的一问（`call_id` 在等，[`Self::driver_asking`]）：照工具调用的确认查（没有规则），记一条决定，`by` 是回答的人，
    /// `cause` 是这个命令；落了盘再回驱动。写了序号的、选了记住的、允许却带了理由附件的，拒绝这个命令。
    pub(super) fn answer_driver(
        &mut self,
        (id, by): (CommandId, By),
        at: Timestamp,
        call_id: CallId,
        answer: Answer,
    ) -> Vec<Action> {
        let Answer::Approval {
            decision,
            reason,
            attachments,
            nested: None,
        } = answer
        else {
            return vec![rejected(id, Reason::NotAsking)];
        };
        let reply = Reply {
            decision,
            reason: reason.filter(|reason| !reason.trim().is_empty()),
            attachments,
        };
        let deny = match reply.judged(false) {
            Ok(deny) => deny,
            Err(reason) => return vec![rejected(id, reason)],
        };
        let Some(token) = self.take_ask(call_id) else {
            return vec![rejected(id, Reason::NotAsking)];
        };
        let answer = match deny {
            true => DriverAnswer::Denied {
                reason: reply.reason.clone(),
            },
            false => DriverAnswer::Allowed,
        };
        let body = ApprovalDecided {
            call_id,
            decision: reply.decision,
            reason: reply.reason,
            attachments: reply.attachments,
            nested: None,
        };
        let decided = self.record(
            at,
            by,
            Some(id.clone()),
            Body::ApprovalDecided(Box::new(body)),
        );
        self.accept(id, vec![decided.seq]);
        self.answer_once_stored(decided.seq, token, answer);
        vec![Action::Append(vec![decided])]
    }

    /// 驱动问的一问到时没人确认：内核替人拒，记一条决定，`by` 是内核，`cause` 是回合的；落了盘再回驱动。不在等的不理。
    pub(super) fn expire_driver(&mut self, at: Timestamp, call_id: CallId) -> Vec<Action> {
        let Some(token) = self.take_ask(call_id) else {
            return Vec::new();
        };
        let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
        let decided = self.record(at, By::Kernel, cause, denied(call_id));
        self.answer_once_stored(decided.seq, token, DriverAnswer::Auto(AutoDeny::Expired));
        vec![Action::Append(vec![decided])]
    }

    /// 这一次请求有了结局（`call/settle.rs`）：还在等人的 `asks` 各记一条拒绝的决定，`by` 是内核，`cause` 是回合的；当场回驱动
    /// 「没了」。交回记的事件。
    pub(super) fn settle_asks(
        &mut self,
        at: Timestamp,
        asks: Vec<Asking>,
        cause: Option<&CommandId>,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        for Asking { token, call_id } in asks {
            events.push(self.record(at, By::Kernel, cause.cloned(), denied(call_id)));
            self.answering.push((
                None,
                Action::AnswerDriver {
                    token,
                    answer: DriverAnswer::Cancelled,
                },
            ));
        }
        events
    }

    /// 落到了第 `upto` 条：决定已经落了盘的回驱动（`session.rs` 的 `stored`）。
    pub(super) fn answers_stored(&mut self, upto: Seq) {
        for (after, _) in &mut self.answering {
            if after.is_some_and(|seq| seq <= upto) {
                *after = None;
            }
        }
    }

    /// 处理完一条输入：不用等落盘的回驱动的话交出去（[`Session::handle`]）。
    pub(super) fn answers_now(&mut self) -> Vec<Action> {
        if self.answering.is_empty() {
            return Vec::new();
        }
        let (now, held): (Vec<_>, Vec<_>) = std::mem::take(&mut self.answering)
            .into_iter()
            .partition(|(after, _)| after.is_none());
        self.answering = held;
        now.into_iter().map(|(_, action)| action).collect()
    }

    /// 决定落到第 `seq` 条以后回 `token` 那一问 `answer`。
    fn answer_once_stored(&mut self, seq: Seq, token: u64, answer: DriverAnswer) {
        let action = Action::AnswerDriver { token, answer };
        self.answering.push((Some(seq), action));
    }

    /// 拿掉在等人的 `call_id` 那一问，交回执行器给的编号；不在等的没有。
    fn take_ask(&mut self, call_id: CallId) -> Option<u64> {
        let Stage::Asking(call) = &mut self.turn.as_mut()?.stage else {
            return None;
        };
        let at = call.asks.iter().position(|ask| ask.call_id == call_id)?;
        Some(call.asks.remove(at).token)
    }
}

/// 拒绝 `call_id` 那一问的决定，没有理由。
fn denied(call_id: CallId) -> Body {
    Body::ApprovalDecided(Box::new(ApprovalDecided {
        call_id,
        decision: Decision::Deny,
        reason: None,
        attachments: Vec::new(),
        nested: None,
    }))
}

//! 被回报叫醒的连着几轮合用一份步数（发布前第三轮审计 R3E-2，`docs/blueprint/kernel/session.md`「回报」第 11 条；2026-11-02
//! 项目主人定）。步数上限原来每一轮各数：她在步数用完以前放一条后台命令、派一个子代理，回报叫醒的下一轮又从头数，没人看着
//! 时一直请求下去也没有总数。
//!
//! - 算进这一串的只有后台命令、子代理的回报和子代理的留言（[`Waker::Job`]）开的回合：她自己放出去的事叫醒她。别的 harness、
//!   别的会话发来的话、定时任务、长期目标的续轮、重启接着干的一轮照旧各算各的，既不算进来，也不让这一串重新算。
//! - 人开的那一轮自己的步数不算：有人看着的会话里它不限，算进来一下就用完了。
//! - 人（`By::is_person`）说一句话，这一串从头算；回合中途插进来的也是，那一轮从这时起不再算进这一串。
//! - 这一串最多几步同不是人开的回合的上限（`Policy::steps_for`）。用完了：正在跑的这一轮这一步齐了就结束（`step_limit`），
//!   之后这样的回报只记下、不开轮，等人开口。
//!
//! 数法只看日志，活着时（`record.rs` 的 `commit`）和载入时（`load/loader.rs`）照同一个 [`Chain::note`] 一条条过，不加事件
//! 字段：一步是这一轮里一次说完了的主请求（`model.called` 是 `ok`、没写到输出上限、不是摘要也不是辅助请求）。重试、写到
//! 上限接着写、撤掉重发都只算说完的那一次，和这一轮的 `requests` 一样不算重来的。

use super::Session;
use super::jobs::Waker;
use crate::event::{Body, CallResult, Event, ModelCalled};
use crate::id::Seq;

/// 这一串用了几步：从日志一条条算。
#[derive(Debug, Default)]
pub(super) struct Chain {
    /// 最近一次开轮以后到的、会叫醒她的回报和留言的序号：开轮时照触发的那一条认这一轮是不是它们叫醒的。回报叫醒的一轮的
    /// 触发总在上一轮开头后面（中途到的接着开、闲着时到的当场开，记在一边的随便哪一轮开了就清掉），所以开轮就清。
    reports: Vec<Seq>,
    /// 这一串里已经结束的几轮用了几步。
    spent: u32,
    /// 正在跑的这一轮算进这一串：它到这时用了几步。不算的没有。
    current: Option<u32>,
}

impl Chain {
    /// 记下追加的一条；`waking` 是它会叫醒她时是谁的（`jobs::waking`）。
    pub(super) fn note(&mut self, event: &Event, waking: Option<&Waker>) {
        if matches!(waking, Some(Waker::Job(_))) {
            self.reports.push(event.seq);
        }
        match &event.body {
            Body::TurnStarted(started) => {
                let woken = started
                    .trigger
                    .is_some_and(|trigger| self.reports.contains(&trigger));
                self.current = woken.then_some(0);
                self.reports.clear();
            }
            Body::MessageUser(_) if event.by.is_person() => {
                self.spent = 0;
                self.current = None;
            }
            Body::ModelCalled(called) if event.turn.is_some() && a_step(called) => {
                if let Some(current) = self.current.as_mut() {
                    *current = current.saturating_add(1);
                }
            }
            Body::TurnEnded(_) => {
                if let Some(current) = self.current.take() {
                    self.spent = self.spent.saturating_add(current);
                }
            }
            _ => {}
        }
    }

    /// 这一串到这时一共用了几步：结束了的几轮，加上正在跑的这一轮。
    fn used(&self) -> u32 {
        self.spent.saturating_add(self.current.unwrap_or(0))
    }
}

/// 这一次请求算一步：说完了的主请求。
fn a_step(called: &ModelCalled) -> bool {
    called.result == CallResult::Ok
        && !called.truncated
        && called.compaction.is_none()
        && called.purpose.is_none()
}

impl Session {
    /// 这一串的步数用完了：同不是人开的回合的上限，不限的永远用不完。
    fn chain_spent(&self) -> bool {
        self.policy
            .steps_for(None)
            .is_some_and(|cap| self.chain.used() >= cap)
    }

    /// 正在跑的这一轮算进这一串、这一串用完了：这一步齐了就结束（`tools.rs` 的 `finish_step`）。
    pub(super) fn chain_runs_out(&self) -> bool {
        self.chain.current.is_some() && self.chain_spent()
    }

    /// `waker` 这一下不开轮、只记下：回报和子代理的留言，这一串用完了的时候。
    pub(super) fn held_back(&self, waker: &Waker) -> bool {
        matches!(waker, Waker::Job(_)) && self.chain_spent()
    }
}

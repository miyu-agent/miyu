//! 扩展驱动在一次调用进行中要人确认（施工 ACP-8，蓝图 `session/driver-ask.md`），执行器这一头：主请求的交给会话问人
//! （[`Asker`]，经 actor 的收件箱），辅助请求、试一家这类没有会话的当场拒；问着的时候空闲时限、总时限都停表，答了接着计。

use std::time::Duration;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use miyu_kernel::id::Seq;
use miyu_models::provider::DriverRef;

use super::super::driver_port::DriverAsk;
use crate::driver_ask::refused;
use crate::port::Back;

/// 主请求 `seen` 问人走哪一路：送回这个会话的 actor。辅助请求没有（[`crate::Reports::asker`]）。
#[derive(Debug, Clone)]
pub(crate) struct Asker {
    pub(crate) seen: Seq,
    pub(crate) back: mpsc::UnboundedSender<Back>,
}

/// 交给 actor 的答的那一头：答了、放掉了都告诉执行器，执行器照它接着计时。
#[derive(Debug)]
pub(crate) struct Answerer {
    reply: Option<oneshot::Sender<Result<Value, String>>>,
    done: mpsc::UnboundedSender<()>,
}

impl Answerer {
    /// 答：回驱动的 `result`，或者参数写得不对的原因。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "端口那一头不等了：驱动这一次已经没了，回的话没人要"
    )]
    pub(crate) fn answer(mut self, answer: Result<Value, String>) {
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(answer);
        }
    }
}

impl Drop for Answerer {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "执行器不等了：这一次已经说完了，不用接着计时"
    )]
    fn drop(&mut self) {
        let _ = self.done.send(());
    }
}

/// 这一次调用里在等人的几问。
pub(super) struct Asking {
    asker: Option<Asker>,
    driver: DriverRef,
    waiting: usize,
    /// 从什么时候起停表：没有在等的没有。
    since: Option<Instant>,
    done: (mpsc::UnboundedSender<()>, mpsc::UnboundedReceiver<()>),
}

impl Asking {
    /// 驱动 `driver` 的这一次，问人走 `asker`（没有的当场拒）。
    pub(super) fn new(asker: Option<Asker>, driver: DriverRef) -> Asking {
        Asking {
            asker,
            driver,
            waiting: 0,
            since: None,
            done: mpsc::unbounded_channel(),
        }
    }

    /// 有在等人的：空闲时限不计。
    pub(super) fn waiting(&self) -> bool {
        self.waiting > 0
    }

    /// 驱动要人确认：主请求的交给 actor、停表；没有会话的当场拒。actor 不在了的放掉，端口回「没了」。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "端口那一头不等了：驱动这一次已经没了"
    )]
    pub(super) fn ask(&mut self, ask: DriverAsk) {
        let DriverAsk { params, reply } = ask;
        let Some(asker) = &self.asker else {
            let _ = reply.send(refused(&params));
            return;
        };
        let answerer = Answerer {
            reply: Some(reply),
            done: self.done.0.clone(),
        };
        let back = Back::DriverAsk {
            seen: asker.seen,
            driver: self.driver.clone(),
            params,
            reply: answerer,
        };
        if asker.back.send(back).is_ok() {
            self.waiting += 1;
            self.since.get_or_insert_with(Instant::now);
        }
    }

    /// 等一问了结（答了、放掉了）；都了结了交回停了多久，还有在等的交回 `None`。
    pub(super) async fn settled(&mut self) -> Option<Duration> {
        self.done.1.recv().await?;
        self.waiting = self.waiting.saturating_sub(1);
        match self.waiting {
            0 => self.since.take().map(|since| since.elapsed()),
            _ => None,
        }
    }
}

//! 闲着的会话不留历史的正文（施工 LW-10 下，`internal/blueprint/session/actor.md`「闲下来收成槽」）：会话歇着（`Session::resting`）
//! 满 [`KEEP`]，内核把有效历史收成槽（`Session::shelve`），拿出来的整条交给放完就退的线程放，再攒一次还给系统
//! （[`crate::heap::trim_soon`]）；下一条要看历史的输入送进内核以前（`Session::needs_history`），照内核说的从日志读回那一截、
//! 放回去（[`Actor::unshelve`]，内核拷一份，读来的同样交出去放）。人说话以后的头一次请求因此先读一遍日志；一个回合里照旧整条在
//! 内存里，每一步不读盘。
//!
//! 计时不轮询：每办完一批，歇着、没收着的记下「最后一次看历史的那一批办完的时刻加 [`KEEP`]」，`run` 的 `select!` 睡到那一刻才醒
//! （没记的不睡）；其间又办了一批看历史的从那一批重新算，不歇着的（回合开了）擦掉。所以闲着的会话收完以后再也不醒。一批里全是
//! 不看历史的输入（群里旁听的话、`events.append` 记的、空闲时的「落了盘」、订阅、换目录，施工 LW-36）不往后推：一直有人说话的
//! 群照样歇够 3 分钟就收，收着时这些也直接送、不放回（`Session::needs_history`）。
//!
//! 读回失败（日志坏了、被截短、读盘出错、读的时候被改短）：会话照旧跑，运行日志记一行 `history unreadable`；内核照旧收着，这时它
//! 看到的是空的历史，这里一个请求都不发（主请求、辅助请求、看图），当场照「没发出去」说完，原话写明读不回、为什么
//! （[`Actor::unreadable`]）：这一轮照没发出去收场，不会拼出少一段的请求。下一次闲着时来了要看历史的输入再读一次，读回来了照常。

use std::time::Duration;

use tokio::time::Instant;

use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::session::Input;

use super::{Actor, Stop};
use crate::TARGET;

/// 歇着满这么久收成槽：3 分钟，和请求体缓存放掉的时刻（施工 LW-3，`route/kept.rs` 的 `KEEP`）、会话闲够退下（`protocol.md`
/// 「会话表」第 9 条）是同一个数。不做成配置项：放掉以后下一次请求多读一遍日志，发出去的字节不变，没有要人来选的。
pub(crate) const KEEP: Duration = crate::route::KEEP;

/// 什么时候收成槽的账（见模块的说明）。
pub(super) struct Rest {
    /// 歇着满这一刻收成槽：没记的不等。
    pub(super) due: Option<Instant>,
    /// 最后一次看历史的那一批办完的时刻：从它起算。
    looked: Instant,
    /// 这一批里有看历史的输入（`needs_history`）、要过上下文的明细：办完时把 `looked` 记成那一刻。造 actor 时是真：造会话、载入
    /// 交回的那一串动作办完就算一批，从那一刻起算，同施工 LW-10。
    seen: bool,
}

impl Rest {
    pub(super) fn new() -> Rest {
        Rest {
            due: None,
            looked: Instant::now(),
            seen: true,
        }
    }
}

impl Actor {
    /// 要送 `input` 了：要看历史的记下这一批看过历史；内核收着的先读回来放回去。读不回的照旧送（见模块的说明）；同一轮里读不回过
    /// 的不再读，等闲着时。读日志的线程 panic 了、写日志的那一份不在了的停下。
    pub(super) async fn unshelve_for(&mut self, input: &Input) -> Result<(), Stop> {
        if !self.session.needs_history(input) {
            return Ok(());
        }
        self.rest.seen = true;
        if self.session.shelved().is_none() {
            return Ok(());
        }
        if self.unread.is_some() && !self.session.idle() {
            return Ok(());
        }
        self.unshelve().await
    }

    /// 头要上下文的明细（`session.context`）：算看过历史，收成了槽的先放回来。
    pub(super) async fn look(&mut self) -> Result<(), Stop> {
        self.rest.seen = true;
        self.unshelve().await
    }

    /// 从日志读回内核要的那一截、放回去。没收着的什么都不做。
    pub(super) async fn unshelve(&mut self) -> Result<(), Stop> {
        let Some(from) = self.session.shelved() else {
            return Ok(());
        };
        let started = std::time::Instant::now();
        let store = self.store.take().ok_or(Stop)?;
        let (store, read) = crate::blocking::once(move || {
            let read = store.events_from(from);
            (store, read)
        })
        .await
        .map_err(|()| {
            tracing::error!(target: TARGET, "panicked, stopped");
            Stop
        })?;
        self.store = Some(store);
        let put = read.and_then(|events| {
            let put = self.session.unshelve(&events);
            // 内核拷了要的那几条，读来的整份是读日志的线程分配的：交给放完就退的线程放，攒一次还给系统。
            crate::blocking::release(events);
            crate::heap::trim_soon();
            put.map_err(|missing| missing.to_string())
        });
        match put {
            Ok(()) => {
                let took_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
                tracing::debug!(target: TARGET, from = from.get(), took_us, "history unshelved");
                self.unread = None;
            }
            Err(why) => {
                tracing::warn!(target: TARGET, from = from.get(), why = why.as_str(), "history unreadable");
                self.unread = Some(why);
            }
        }
        Ok(())
    }

    /// 一批办完了：这一批看过历史的，从这一刻起算；没看过的照上一次看历史的那一批算，不往后推（施工 LW-36）。歇着、没收着的满
    /// [`KEEP`] 收；不歇着的、收着的不等。
    pub(super) fn rest_later(&mut self) {
        if std::mem::take(&mut self.rest.seen) {
            self.rest.looked = Instant::now();
        }
        let looked = self.rest.looked;
        self.rest.due =
            (self.session.resting() && self.session.shelved().is_none()).then(|| looked + KEEP);
    }

    /// 到点了：还歇着的收成槽，拿出来的整条交给放完就退的线程放、攒一次还给系统。
    pub(super) fn rest(&mut self) {
        self.rest.due = None;
        if let Some(taken) = self.session.shelve() {
            tracing::debug!(target: TARGET, events = taken.len(), "history shelved");
            crate::blocking::release(taken);
            crate::heap::trim_soon();
        }
    }

    /// 内核收着、历史读不回来的时候：请求一律不发，交回当场说完用的错（见模块的说明）。没收着的没有。
    pub(super) fn unreadable(&self) -> Option<CallError> {
        self.session.shelved()?;
        let why = self.unread.as_deref().unwrap_or("not read back yet");
        Some(CallError {
            class: ErrorClass::Unclassified,
            message: format!("这个会话的历史从日志读不回来，请求没有发出去：{why}"),
            status: None,
            retry_after_ms: None,
        })
    }
}

//! 一次模型请求每一段用了多久（施工 YC-14，`docs/blueprint/session/actor.md`「运行日志」的 `request timed`）：长会话里每一步
//! 核心自己花的时间跟着上下文涨（压测 2026-11-10：请求体 700 KB 时「工具结束到出 `call_model`」3.67 毫秒、「`call_model` 到
//! HTTP 发出去」2.33 毫秒），要看慢在哪一段。
//!
//! 只在这个来源调到 `DEBUG` 时量（[`on`]）：平常一次请求只多问几回「有没有人听」，不读时钟。内核是纯逻辑、不许读时钟，
//! 所以它那一段在会话 actor 这里包着量（[`handle`]），随回报（`port.rs` 的 `Reports`）带到发的任务里，编完、交给 HTTP 执行器
//! 之前和这一头量的合成一行（[`Watch::sent`]）。量的都是用时和个数，没有对话的字。

use std::time::Instant;

use miyu_kernel::id::Seq;
use miyu_kernel::session::{Action, Input, Session};

use crate::TARGET;

/// 这个来源的 `DEBUG` 有没有人听：没有的什么都不量。
fn on() -> bool {
    tracing::enabled!(target: TARGET, tracing::Level::DEBUG)
}

/// 从 `from` 到这一刻多少微秒。
fn micros(from: Instant) -> u64 {
    u64::try_from(from.elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// 内核那一头量的：出 `CallModel` 的那一次 `Session::handle`。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Kernel {
    /// 那一次 `handle` 整个用了多久：组装、看图、判线（估用量好几遍）、算指纹都在里面。
    kernel_us: u64,
    /// 照有效历史再组装一遍用了多久。
    assemble_us: u64,
    /// 再估一遍这份请求的用量用了多久；一次 `handle` 里要估三四遍（熔断、压回答、挑成员、起压线）。
    estimate_us: u64,
    /// 再算一遍这份请求的指纹用了多久。
    fingerprint_us: u64,
    /// 请求里几条消息。
    messages: usize,
}

/// 把 `input` 送进内核，交回它出的动作。`DEBUG` 开着、出了 `CallModel` 的，量好了放进 `slot`，交给端口时带上
/// （`actor/model.rs` 的 `call`）。后三格是出了动作以后照同一份历史、同一份请求各重跑一遍量的（内核里不能读时钟），不在
/// `kernel_us` 里；重跑的只读，结果扔掉，内核的状态、发出去的请求都不变。
pub(crate) fn handle(
    session: &mut Session,
    input: Input,
    slot: &mut Option<Kernel>,
) -> Vec<Action> {
    if !on() {
        return session.handle(input);
    }
    let started = Instant::now();
    let actions = session.handle(input);
    let kernel_us = micros(started);
    let asked = actions.iter().find_map(|action| match action {
        Action::CallModel { request, .. } => Some(request),
        _ => None,
    });
    if let Some(request) = asked {
        let started = Instant::now();
        drop(session.assembled());
        let assemble_us = micros(started);
        let started = Instant::now();
        let _ = session.used(request);
        let estimate_us = micros(started);
        let started = Instant::now();
        let _ = request.fingerprint();
        *slot = Some(Kernel {
            kernel_us,
            assemble_us,
            estimate_us,
            fingerprint_us: micros(started),
            messages: request.messages.len(),
        });
    }
    actions
}

/// 随回报带着走的那一半：哪一次请求、交给端口的那一刻、内核那一头量的。`DEBUG` 关着的、不经会话发的（一次性入口）是空的。
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Lead {
    seen: Option<u64>,
    at: Option<Instant>,
    kernel: Option<Kernel>,
}

impl Lead {
    /// 请求 `seen` 这就交给端口。
    pub(crate) fn start(seen: Seq) -> Lead {
        Lead {
            seen: Some(seen.get()),
            at: on().then(Instant::now),
            kernel: None,
        }
    }

    /// 带上内核那一头量的：主请求有，辅助请求没有。
    pub(crate) fn with(mut self, kernel: Option<Kernel>) -> Lead {
        self.kernel = kernel;
        self
    }
}

/// 发的任务里量的：从交给端口到开始发（挑端点、排到这个任务）、取 blob、编码、放掉请求。
pub(crate) struct Watch {
    lead: Lead,
    /// 上一段在哪一刻结束；`DEBUG` 关着的没有，什么都不量、不记。
    last: Option<Instant>,
    wait_us: Option<u64>,
    blobs_us: u64,
    encode_us: u64,
    drop_us: u64,
}

impl Watch {
    /// 开始发：`lead` 是回报带来的那一半。
    pub(crate) fn start(lead: Lead) -> Watch {
        Watch {
            lead,
            last: on().then(Instant::now),
            wait_us: lead.at.map(micros),
            blobs_us: 0,
            encode_us: 0,
            drop_us: 0,
        }
    }

    /// 上一段到这一刻多少微秒，下一段从这一刻算。
    fn lap(&mut self) -> u64 {
        let Some(last) = self.last else {
            return 0;
        };
        let now = Instant::now();
        self.last = Some(now);
        u64::try_from(now.duration_since(last).as_micros()).unwrap_or(u64::MAX)
    }

    /// 编码要的 blob 列好、取好了。
    pub(crate) fn fetched(&mut self) {
        self.blobs_us = self.lap();
    }

    /// 编成片段清单了：补占位工具、压输出上限也在这一段里。
    pub(crate) fn encoded(&mut self) {
        self.encode_us = self.lap();
    }

    /// 请求、取来的字节放掉了。
    pub(crate) fn dropped(&mut self) {
        self.drop_us = self.lap();
    }

    /// 这就交给 HTTP 执行器，请求体 `bytes` 个字节：记那一行。请求体的哈希不在这以前算：跟着送出去的字节一路算（`miyu-http`
    /// 的 `body.rs`）。
    pub(crate) fn sent(self, bytes: u64) {
        if self.last.is_none() {
            return;
        }
        let kernel = self.lead.kernel;
        tracing::debug!(
            target: TARGET,
            seen = self.lead.seen,
            kernel_us = kernel.map(|kernel| kernel.kernel_us),
            assemble_us = kernel.map(|kernel| kernel.assemble_us),
            estimate_us = kernel.map(|kernel| kernel.estimate_us),
            fingerprint_us = kernel.map(|kernel| kernel.fingerprint_us),
            messages = kernel.map(|kernel| kernel.messages),
            wait_us = self.wait_us,
            blobs_us = self.blobs_us,
            encode_us = self.encode_us,
            drop_us = self.drop_us,
            bytes,
            "request timed"
        );
    }
}

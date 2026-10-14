//! 核心自己记内存账（`docs/designs/23-性能预算.md` F4，施工 V-4，`docs/blueprint/heap.md`「自己记账」）：起来时、之后每
//! [`Timing::every`] 采一次自己的内存（RSS；Linux 上另有 PSS、私有的、匿名的、换出去的，macOS 上另有算在头上的；glibc 的堆；载入着几个会话），一行一个 JSON 写进
//! `state/logs/memory.jsonl`；一阵请求过去、安静下来时看一眼 RSS，比上一次采的时候涨了 [`Timing::grew`] 的多采一次（施工
//! V-6 下，以前两次之间每五秒看一眼）。
//! 量不出来的平台（Windows，23「后续再定」）不记。

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use serde_json::{Map, Value, json};

use miyu_endpoint::Core;
use miyu_heap::Stats;
use miyu_heap::usage::Usage;
use miyu_kernel::time::Timestamp;
use miyu_log::{RotatingFile, Sink};

use crate::TARGET;

/// 账记在 `state/logs/` 下的这个文件里。
pub const FILE: &str = "memory.jsonl";

/// 一份写到多大换下：换下的是 `memory.jsonl.1`，只留这一份。一分钟一行、一行三百来字节，一份够记两天多。
pub const LIMIT: u64 = 1024 * 1024;

/// 多久采一次、涨多少算涨得快。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// 隔多久采一次。
    pub every: Duration,
    /// 一阵请求过去、安静下来时（施工 V-6 下，`settle.rs`），RSS 比上一次采的时候涨了这么多字节，多采一次。
    pub grew: u64,
}

impl Default for Timing {
    /// 一分钟采一次，涨 32 MiB 多采：打开一个一万条事件的大会话约涨 20 MB（`docs/perf`），比它大的才算涨得快。以前两次之间
    /// 每五秒看一眼 RSS（施工 V-4），空闲的核心每秒因此醒一次多，施工 V-6 下改成安静下来时看。
    fn default() -> Timing {
        Timing {
            every: Duration::from_secs(60),
            grew: 32 * 1024 * 1024,
        }
    }
}

/// 为什么采这一次。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// 起来时。
    Start,
    /// 到点了。
    Tick,
    /// 涨得快。
    Grew,
}

impl Why {
    /// 写进那一行的说法。
    pub fn as_str(self) -> &'static str {
        match self {
            Why::Start => "start",
            Why::Tick => "tick",
            Why::Grew => "grew",
        }
    }
}

/// 涨得快不快：上一次采的时候 RSS 是 `was`，这时是 `now`。
pub fn grew(was: u64, now: u64, timing: &Timing) -> bool {
    now >= was.saturating_add(timing.grew)
}

/// 一行（不带换行）：格照名字的字母先后，字节数都是整数。这个平台量不出的那几样不写（`Usage` 里没有的），没有堆的统计的
/// （不是 glibc）不写 `heap`。
pub fn line(
    at: Timestamp,
    why: Why,
    usage: &Usage,
    heap: Option<&Stats>,
    sessions: usize,
) -> String {
    let mut fields = Map::new();
    let mut put = |key: &str, value: Option<Value>| {
        if let Some(value) = value {
            fields.insert(key.to_string(), value);
        }
    };
    put("anon", usage.anon.map(Value::from));
    put("at", Some(json!(at)));
    put("footprint", usage.footprint.map(Value::from));
    put(
        "heap",
        heap.map(|heap| {
            json!({
                "arenas": heap.arenas,
                "free": heap.free,
                "mmapped": heap.mmapped,
                "used": heap.used,
            })
        }),
    );
    put("private", usage.private.map(Value::from));
    put("pss", usage.pss.map(Value::from));
    put("rss", Some(Value::from(usage.rss)));
    put("sessions", Some(Value::from(sessions)));
    put("swap", usage.swap.map(Value::from));
    put("why", Some(Value::from(why.as_str())));
    Value::Object(fields).to_string()
}

/// 起来以后的记账：账本，和到点采的那个任务。
pub struct Sampling {
    /// 账本：安静下来时照它看涨得快不快（[`Ledger::settled`]）。
    pub ledger: Arc<Ledger>,
    /// 起来时采一次、之后到点采的任务。
    pub task: tokio::task::JoinHandle<()>,
}

/// 起采样：量不出来的平台不起，交回没有。`logs` 是 `state/logs/`，打不开文件的记一行 `WARN`、不起。要在 tokio 的运行时里调。
pub fn spawn(core: &Arc<Core>, logs: &Path, timing: Timing) -> Option<Sampling> {
    if miyu_heap::usage::resident().is_none() {
        tracing::debug!(target: TARGET, "memory samples unsupported");
        return None;
    }
    let sink = match RotatingFile::open_file(logs, FILE, LIMIT, 1) {
        Ok(sink) => Arc::new(sink),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "memory samples not started");
            return None;
        }
    };
    let ledger = Arc::new(Ledger {
        core: Arc::downgrade(core),
        sink,
        timing,
        last: AtomicU64::new(0),
    });
    let task = tokio::spawn(Arc::clone(&ledger).tick());
    Some(Sampling { ledger, task })
}

/// 记账的那一份。
pub struct Ledger {
    core: Weak<Core>,
    sink: Arc<RotatingFile>,
    timing: Timing,
    /// 上一次采的时候 RSS 多大。
    last: AtomicU64,
}

impl Ledger {
    /// 起来时采一次，之后每 [`Timing::every`] 采一次，一直采到核心没了。
    async fn tick(self: Arc<Ledger>) {
        let mut why = Why::Start;
        while self.take(why).await {
            tokio::time::sleep(self.timing.every).await;
            why = Why::Tick;
        }
    }

    /// 一阵请求过去、安静下来了（施工 V-6 下，`settle.rs` 在还了内存以后叫它）：RSS 比上一次采的时候涨了
    /// [`Timing::grew`] 以上的，多采一次。不定时看。
    pub async fn settled(&self) {
        let now = tokio::task::spawn_blocking(miyu_heap::usage::resident)
            .await
            .ok()
            .flatten();
        if now.is_some_and(|now| grew(self.last.load(Ordering::Acquire), now, &self.timing)) {
            self.take(Why::Grew).await;
        }
    }

    /// 采一次、写一行，记下这时的 RSS。核心没了的什么都不写，交回假。
    async fn take(&self, why: Why) -> bool {
        let Some(sessions) = loaded(&self.core).await else {
            return false;
        };
        let sink = Arc::clone(&self.sink);
        let taken = tokio::task::spawn_blocking(move || take(&sink, why, sessions)).await;
        if let Some(rss) = taken.ok().flatten() {
            self.last.store(rss, Ordering::Release);
        }
        true
    }
}

/// 安静下来以后看涨得快不快（施工 V-6 下）：交给 `settle.rs`。没有账本的（量不出来的平台）什么都不做。
pub(crate) fn on_settled(ledger: Option<Arc<Ledger>>) -> crate::settle::Then {
    Box::new(move || {
        let ledger = ledger.clone();
        Box::pin(async move {
            if let Some(ledger) = ledger {
                ledger.settled().await;
            }
        })
    })
}

/// 核心还在的话，这时载入着几个会话。
async fn loaded(core: &Weak<Core>) -> Option<usize> {
    let core = core.upgrade()?;
    Some(core.loaded().await)
}

/// 采一次、写一行，交回这时的 RSS；量不出来的什么都不写。在阻塞线程里调。
fn take(sink: &RotatingFile, why: Why, sessions: usize) -> Option<u64> {
    let usage = miyu_heap::usage::usage()?;
    let heap = miyu_heap::stats();
    sink.write_line(&line(
        crate::trash::now(),
        why,
        &usage,
        heap.as_ref(),
        sessions,
    ));
    Some(usage.rss)
}

#[cfg(test)]
mod tests;

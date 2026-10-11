//! 核心自己记内存账（`docs/designs/23-性能预算.md` F4，施工 V-4，`docs/blueprint/heap.md`「自己记账」）：起来时、之后每
//! [`Timing::every`] 采一次自己的内存（RSS；Linux 上另有 PSS、私有的、匿名的、换出去的，macOS 上另有算在头上的；glibc 的堆；载入着几个会话），一行一个 JSON 写进
//! `state/logs/memory.jsonl`；两次之间每 [`Timing::peek`] 看一眼 RSS，比上一次采的时候涨了 [`Timing::grew`] 的当场多采一次。
//! 量不出来的平台（Windows，23「后续再定」）不记。

use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

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

/// 多久采一次、多久看一眼、涨多少算涨得快。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// 隔多久采一次。
    pub every: Duration,
    /// 两次之间隔多久看一眼 RSS。
    pub peek: Duration,
    /// RSS 比上一次采的时候涨了这么多字节，当场多采一次。
    pub grew: u64,
}

impl Default for Timing {
    /// 一分钟采一次，五秒看一眼，涨 32 MiB 多采：打开一个一万条事件的大会话约涨 20 MB（`docs/perf`），比它大的才算涨得快。
    fn default() -> Timing {
        Timing {
            every: Duration::from_secs(60),
            peek: Duration::from_secs(5),
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

/// 该不该采：上一次采是 `since` 以前、那时 RSS 是 `was`；这时看到的 RSS 是 `now`（看不到的是没有）。到点了是 `Tick`，
/// 没到点、涨够了的是 `Grew`。
pub fn due(since: Duration, was: u64, now: Option<u64>, timing: &Timing) -> Option<Why> {
    if since >= timing.every {
        return Some(Why::Tick);
    }
    now.filter(|rss| *rss >= was.saturating_add(timing.grew))
        .map(|_| Why::Grew)
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

/// 起采样：量不出来的平台不起，交回没有。`logs` 是 `state/logs/`，打不开文件的记一行 `WARN`、不起。要在 tokio 的运行时里调。
pub fn spawn(core: &Arc<Core>, logs: &Path, timing: Timing) -> Option<tokio::task::JoinHandle<()>> {
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
    Some(tokio::spawn(watch(Arc::downgrade(core), sink, timing)))
}

/// 一直采到核心没了。
async fn watch(core: Weak<Core>, sink: Arc<RotatingFile>, timing: Timing) {
    let mut why = Why::Start;
    loop {
        let Some(sessions) = loaded(&core).await else {
            return;
        };
        let writer = Arc::clone(&sink);
        let taken = tokio::task::spawn_blocking(move || take(&writer, why, sessions)).await;
        let was = taken.ok().flatten().unwrap_or(0);
        let since = Instant::now();
        why = loop {
            tokio::time::sleep(timing.peek).await;
            let now = tokio::task::spawn_blocking(miyu_heap::usage::resident)
                .await
                .ok()
                .flatten();
            if let Some(why) = due(since.elapsed(), was, now, &timing) {
                break why;
            }
        };
    }
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

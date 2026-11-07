//! 工具调用后的通知（施工 HK-2，设计 `65-工具挂接点.md` 第四节，蓝图 `extensions.md`「工具调用后的通知」）：清单写了 `[after]`、
//! 声明了 `events.read` 的扩展包，主人批了、开着、在跑的，在会话的一次调用的结果落了盘以后收到一条只读的 `tool.after`。
//!
//! - 表（[`Afters`]）：照核心手上的清单建（`table.rs`，同守卫），结果落盘时会话先查一次表（[`covered`]），没有收它的不写通知。
//! - 发给谁（[`send`]）：表对上了的，再看开关开着、批了 `events.read`、碰得到这个会话（同订事件流，`sessions::reaches`），
//!   握过手的连接在（没在跑的、还没握手的不发，不补）。扩展自己登记的工具也发给它。
//! - 不等、不挡：每个扩展一条 256 条的队列（[`QUEUE`]），会话那一头只入队（`try_send`），满了丢新的；一段连着丢的头一条记一行
//!   `WARN`，带按扩展累计丢了几条，有了空位再记一行。每条队列一个发送任务，往连接的写队列里送，等得起空位，不拖会话。连接断了、
//!   扩展崩了，队列和任务一起丢掉；重启以后新开一条空的。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

use miyu_config::package::{After, Capability};
use miyu_store::packages::Found;
use miyu_tool::{AfterNote, AfterOutcome};

use crate::Core;
use crate::reverse::Peer;

/// 运行日志的目标。
const TARGET: &str = "miyu::endpoint";

/// 一个扩展的队列至多攒几条（设计 65 第四节第 4 条）。
pub(crate) const QUEUE: usize = 256;

/// 表里的一个收件人：哪个包、收哪些调用的。
#[derive(Debug, Clone)]
struct Entry {
    package: String,
    after: After,
}

/// 一个扩展的队列：发给哪个连接、往发送任务那一头放、累计丢了几条、这一阵是不是正在丢。
struct Outlet {
    peer: Peer,
    queue: mpsc::Sender<Value>,
    dropped: u64,
    dropping: bool,
}

/// 通知的表和每个扩展的队列：核心一份。
#[derive(Default)]
pub(crate) struct Afters {
    built: super::table::Table<Entry>,
    outlets: Mutex<BTreeMap<String, Outlet>>,
}

impl Afters {
    /// 照核心这时的清单拿表。
    fn table(&self, core: &Core) -> Arc<Vec<Entry>> {
        self.built.get(core, entries)
    }

    /// 核心起来时建好表。
    pub(crate) fn build(&self, core: &Core) {
        let table = self.table(core);
        if !table.is_empty() {
            let packages: Vec<&str> = table.iter().map(|entry| entry.package.as_str()).collect();
            tracing::info!(target: TARGET, ?packages, "tool.after listeners");
        }
    }
}

/// 清单里写了 `[after]` 的扩展包，照包的编号排（读清单时已经查过声明了 `events.read`）。
fn entries(packages: &[Found]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = super::processes(packages)
        .filter(|(_, manifest)| super::declared(manifest).contains(&Capability::EventsRead))
        .filter_map(|(id, manifest)| {
            Some(Entry {
                package: id.to_string(),
                after: manifest.after.clone()?,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.package.cmp(&b.package));
    entries.dedup_by(|a, b| a.package == b.package);
    entries
}

/// 工具 `tool`、访问类别 `access` 的调用表里有没有收件人：只查内存里的表。
pub(crate) fn covered(core: &Core, tool: &str, access: Option<&str>) -> bool {
    core.extensions
        .afters
        .table(core)
        .iter()
        .any(|entry| entry.after.covers(tool, access))
}

/// 发一条（见模块的说明）：只入队，不等。
pub(crate) fn send(core: &Core, note: &AfterNote) {
    let table = core.extensions.afters.table(core);
    let covering: Vec<&Entry> = table
        .iter()
        .filter(|entry| entry.after.covers(&note.tool, note.access.as_deref()))
        .collect();
    if covering.is_empty() {
        return;
    }
    let (switches, _) = super::read_switches(core);
    let packages = core.system_packages();
    let mut params = None;
    for entry in covering {
        let held = packages
            .iter()
            .find(|found| found.id == entry.package)
            .and_then(|found| found.read.as_ref().ok())
            .is_some_and(|manifest| {
                super::on(&switches, &entry.package, manifest)
                    && super::approval::holds(core, &entry.package, manifest, &switches)
                        .contains(&Capability::EventsRead)
            });
        // 场所会话照订事件流的规矩（第四节第 5 条，core1 2026-11-07 照推荐定）。
        if !held
            || !crate::sessions::reaches(core, Some(&entry.package), &note.venue, note.confined)
        {
            continue;
        }
        let params = params.get_or_insert_with(|| params_of(note));
        core.extensions
            .afters
            .put(&entry.package, core.provided.linked(&entry.package), params);
    }
}

impl Afters {
    /// 往包 `package` 的队列放一条：没在跑的（没有握过手的连接）不发，丢掉它的队列；连接换了（重启过）的新开一条。
    fn put(&self, package: &str, peer: Option<Peer>, params: &Value) {
        let mut outlets = self.outlets.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(peer) = peer else {
            outlets.remove(package);
            return;
        };
        let stale = outlets
            .get(package)
            .is_none_or(|outlet| !outlet.peer.same(&peer) || outlet.queue.is_closed());
        if stale {
            outlets.insert(package.to_string(), open(peer));
        }
        let Some(outlet) = outlets.get_mut(package) else {
            return;
        };
        match outlet.queue.try_send(params.clone()) {
            Ok(()) => {
                if outlet.dropping {
                    outlet.dropping = false;
                    tracing::info!(target: TARGET, package, dropped = outlet.dropped, "tool.after queue has room again");
                }
            }
            Err(TrySendError::Full(_)) => {
                outlet.dropped += 1;
                if !outlet.dropping {
                    outlet.dropping = true;
                    tracing::warn!(target: TARGET, package, dropped = outlet.dropped, "tool.after queue full, dropping new ones");
                }
            }
            // 发送任务刚退（连接刚断）：这一条不发，下一条照新的连接开。
            Err(TrySendError::Closed(_)) => {
                outlets.remove(package);
            }
        }
    }
}

/// 开一条队列和它的发送任务：一条一条往连接的写队列送，等得起空位；连接断了就退，队列跟着丢掉。
fn open(peer: Peer) -> Outlet {
    let (queue, mut taken) = mpsc::channel::<Value>(QUEUE);
    let sending = peer.clone();
    tokio::spawn(async move {
        while let Some(params) = taken.recv().await {
            if !sending.notify_waiting("tool.after", params).await {
                return;
            }
        }
    });
    Outlet {
        peer,
        queue,
        dropped: 0,
        dropping: false,
    }
}

/// 通知的参数（设计 65 第四节第 3 条）：直接调的带 `blocks`，脚本里调的带 `result`、`bytes`（有的才写）；没有访问类别、没真
/// 执行过的不写那一格。
fn params_of(note: &AfterNote) -> Value {
    let mut params = json!({
        "session": note.session,
        "call_id": note.call_id,
        "tool": note.tool,
        "status": note.status,
        "by": note.by,
        "args": note.args,
    });
    if let Some(access) = &note.access {
        params["access"] = json!(access);
    }
    if let Some(ms) = note.duration_ms {
        params["duration_ms"] = json!(ms);
    }
    match &note.outcome {
        AfterOutcome::Blocks(blocks) => params["blocks"] = blocks.clone(),
        AfterOutcome::Stored { result, bytes } => {
            if let Some(result) = result {
                params["result"] = json!(result);
            }
            if let Some(bytes) = bytes {
                params["bytes"] = json!(bytes);
            }
        }
    }
    params
}

#[cfg(test)]
mod tests;

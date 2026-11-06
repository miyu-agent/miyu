//! 工具调用前的守卫（施工 HK-1，设计 `65-工具挂接点.md` 第三节，蓝图 `extensions.md`「工具调用前的守卫」）：清单写了 `[guard]`、
//! 声明了 `tool.guard` 的扩展包，主人批了、开着的，在会话的每一次调用经过权限策略以后被问一句跑不跑。
//!
//! - 表（[`Guards`]）：照核心手上的清单建，核心起来时建一次，清单换了（装卸、升级）照新的那一份重建；调用前查一次表，没有挂在
//!   这次调用上的照现在的路走（第六节）。开关、批没批在表对上以后才读：表里没有的调用一点不多。
//! - 问（[`ask`]）：挂着的几个并行问 `tool.guard`、各自照清单的时限，照包的编号合：有一个拒绝就拒绝，记编号在前的那一个的理由；
//!   没有拒绝、有一个要问人的问人；都放行的放行。超时、连接断了、回错误、答得不对的，开着却没在跑的（正在起、刚崩、起不来），
//!   都算拒绝（第五条）。丢掉这个 future（打断、回合被取消）时，还没答的发 `guard.cancel`（第六条）；超时的同样发。
//! - 拒绝的理由是第三方写的字（第三节「拒绝的理由」）：截到 300 个字，照模板转义（`miyu_kernel::template`）套进出厂的
//!   `core/guard/blocked.txt`，`texts.rs`。

mod answer;
mod texts;

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::task::JoinSet;

use miyu_config::package::{Capability, Guard};
use miyu_kernel::id::ModuleId;
use miyu_store::packages::Found;
use miyu_tool::{GuardAsk, Guarded};

use crate::Core;
use crate::reverse::Peer;
use answer::Answer;

/// 运行日志的目标。
const TARGET: &str = "miyu::endpoint";

/// 表里的一个守卫：哪个包、问哪些调用、等多久。
#[derive(Debug, Clone)]
struct Entry {
    package: String,
    module: ModuleId,
    guard: Guard,
}

/// 建好的表，连同照它建的那一份清单（`Arc` 认是不是同一份）。
type Built = (Arc<Vec<Found>>, Arc<Vec<Entry>>);

/// 守卫的表：核心一份。
#[derive(Default)]
pub(crate) struct Guards {
    built: Mutex<Option<Built>>,
}

impl Guards {
    /// 照核心这时的清单拿表：和上一次是同一份的照旧，换了的重建。
    fn table(&self, core: &Core) -> Arc<Vec<Entry>> {
        let packages = core.system_packages();
        let mut built = self.built.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((seen, table)) = built.as_ref()
            && Arc::ptr_eq(seen, &packages)
        {
            return Arc::clone(table);
        }
        let table = Arc::new(entries(&packages));
        *built = Some((packages, Arc::clone(&table)));
        table
    }

    /// 核心起来时建好表（设计 65 第三节第 2 条）。
    pub(crate) fn build(&self, core: &Core) {
        let table = self.table(core);
        if !table.is_empty() {
            let packages: Vec<&str> = table.iter().map(|entry| entry.package.as_str()).collect();
            tracing::info!(target: TARGET, ?packages, "tool guards");
        }
    }
}

/// 清单里的守卫，照包的编号排：扩展包、写了 `[guard]`、声明了 `tool.guard` 的（读清单时已经查过两样一起写）。
fn entries(packages: &[Found]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = super::processes(packages)
        .filter(|(_, manifest)| super::declared(manifest).contains(&Capability::ToolGuard))
        .filter_map(|(id, manifest)| {
            Some(Entry {
                package: id.to_string(),
                module: ModuleId::parse(id).ok()?,
                guard: manifest.guard.clone()?,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.package.cmp(&b.package));
    entries.dedup_by(|a, b| a.package == b.package);
    entries
}

/// 挂在工具 `tool`、访问类别 `access` 的调用上的守卫，照包的编号排：表里对得上，工具不是它自己登记的（第三节第 2 条），
/// 开关开着、主人批了 `tool.guard` 的。主人关掉的不问。
fn matching(core: &Core, tool: &str, access: &str) -> Vec<Entry> {
    let table = core.extensions.guards.table(core);
    let covering: Vec<&Entry> = table
        .iter()
        .filter(|entry| entry.guard.covers(tool, access))
        .collect();
    if covering.is_empty() {
        return Vec::new();
    }
    let owner = core.tools().package_of(tool).map(str::to_string);
    let (switches, _) = super::read_switches(core);
    let packages = core.system_packages();
    covering
        .into_iter()
        .filter(|entry| owner.as_deref() != Some(entry.package.as_str()))
        .filter(|entry| {
            packages
                .iter()
                .find(|found| found.id == entry.package)
                .and_then(|found| found.read.as_ref().ok())
                .is_some_and(|manifest| {
                    super::on(&switches, &entry.package, manifest)
                        && super::approval::holds(core, &entry.package, manifest, &switches)
                            .contains(&Capability::ToolGuard)
                })
        })
        .cloned()
        .collect()
}

/// 这次调用上有没有挂着守卫：会话照它决定问不问。
pub(crate) fn watched(core: &Core, tool: &str, access: &str) -> bool {
    !matching(core, tool, access).is_empty()
}

/// 问挂在这次调用上的守卫，照包的编号合（见模块的说明）。
pub(crate) async fn ask(core: Arc<Core>, ask: GuardAsk) -> Guarded {
    let entries = matching(&core, &ask.tool, &ask.access);
    if entries.is_empty() {
        return Guarded::Allow;
    }
    let args: Value = serde_json::from_str(&ask.args).unwrap_or_else(|_| json!({}));
    let params = json!({
        "session": ask.session,
        "call_id": ask.call_id,
        "tool": ask.tool,
        "args": args,
        "access": ask.access,
        "cwd": ask.cwd,
        "owner": ask.owner,
        "venue": ask.venue.as_str(),
    });
    let cancel = json!({"session": ask.session, "call_id": ask.call_id});
    // 放在 `JoinSet` 里：这个 future 被丢掉时一起掐掉，各自的 `Cancel` 跟着发 `guard.cancel`。
    let mut asking = JoinSet::new();
    for (k, entry) in entries.iter().enumerate() {
        let peer = core.provided.linked(&entry.package);
        let wait = Duration::from_millis(entry.guard.timeout_ms);
        let (params, cancel) = (params.clone(), cancel.clone());
        asking.spawn(async move { (k, one(peer, params, cancel, wait).await) });
    }
    let mut answers: Vec<Option<Answer>> = vec![None; entries.len()];
    while let Some(done) = asking.join_next().await {
        if let Ok((k, answer)) = done {
            answers[k] = Some(answer);
        }
    }
    settle(&entries, answers, &ask.tool)
}

/// 问一个守卫：没在跑的不问；超时的、丢掉时还没答的发 `guard.cancel`。
async fn one(peer: Option<Peer>, params: Value, cancel: Value, wait: Duration) -> Answer {
    let Some(peer) = peer else {
        return Answer::Absent;
    };
    let mut cancel = Cancel {
        peer: peer.clone(),
        params: cancel,
        armed: true,
    };
    let answer = tokio::time::timeout(wait, peer.call("tool.guard", params)).await;
    let Ok(answer) = answer else {
        return Answer::TimedOut;
    };
    cancel.armed = false;
    match answer {
        Ok(Ok(result)) => answer::read(result),
        Ok(Err(_)) => Answer::Failed,
        Err(_) => Answer::Absent,
    }
}

/// 照包的编号合（第三节第 4 条）：拒绝的（没答、答得不对的也算）编号在前的那一个；没有的，要问人的编号在前的那一个；都放行的
/// 放行。任务自己没了的（不会发生）照出错算，往严里走。
fn settle(entries: &[Entry], answers: Vec<Option<Answer>>, tool: &str) -> Guarded {
    let answers: Vec<Answer> = answers
        .into_iter()
        .map(|answer| answer.unwrap_or(Answer::Failed))
        .collect();
    for (entry, answer) in entries.iter().zip(&answers) {
        if let Some(why) = answer.failure() {
            tracing::warn!(target: TARGET, package = entry.package, why, "tool.guard counted as a denial");
        }
    }
    let pairs = || entries.iter().zip(&answers);
    if let Some((entry, answer)) = pairs().find(|(_, answer)| answer.denies()) {
        return Guarded::Deny {
            module: entry.module.clone(),
            worded: texts::denied(&entry.package, answer),
        };
    }
    if let Some((entry, Answer::Ask(reason))) =
        pairs().find(|(_, answer)| matches!(answer, Answer::Ask(_)))
    {
        return Guarded::Ask {
            module: entry.module.clone(),
            detail: texts::asking(&entry.package, tool, reason.as_deref()),
        };
    }
    Guarded::Allow
}

/// 发 `guard.cancel` 的：超时的、丢掉时还没答的发一次（同 `tool.cancel` 的写法，`provide/remote.rs`）。
struct Cancel {
    peer: Peer,
    params: Value,
    armed: bool,
}

impl Drop for Cancel {
    fn drop(&mut self) {
        if self.armed && !self.peer.notify("guard.cancel", self.params.clone()) {
            tracing::debug!(target: TARGET, "guard.cancel not sent");
        }
    }
}

#[cfg(test)]
mod tests;

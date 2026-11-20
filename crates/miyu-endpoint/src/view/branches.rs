//! 场所会话的并行支线（施工 VW-2，`internal/blueprint/view.md`「会话状态」的 `jobs`）：桥照 `session.fork {seal_after: 1}` 从主线
//! 分出去、只跑一轮的那几条，像子代理一样作为主线会话状态 `jobs` 里的一项出来，头的右侧边栏现成画它、点得开。
//!
//! - 主线的日志里没有分叉的记录：照子会话日志的第一条 `session.created` 认（父会话是它、`forked` 写着从它分出来、不是 fork
//!   子代理）；场所会话不进会话列表的索引，只能照日志。子代理也是子会话，认出来不是支线的记下、不再读。
//! - 一项 `{job, what: "agent", fork: true, session, title, state, started, ended?}`：`job`、`session` 都是支线的会话编号；
//!   `title` 照会话列表的算法（起了标题的写标题，不然写第一句话的头一行）；`state` 是 `running`（在跑一轮，或者还没跑完第一轮）、
//!   `done`、`stopped`（最后一轮是被打断、断掉的）；`ended` 是最后一轮结束的时刻。
//! - 在跑的都列，做完的只留最近的 [`KEPT`] 条（同任务表做完的留 20 个）：主群一天三四十条支线，全列状态一推就是几十 KB。
//! - 子会话动了（会话列表报的）才重读那一条；没见过的先照它日志的第一条认父会话。只有场所会话量这个。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, EndReason, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;
use miyu_store::index::Row;
use miyu_store::root::DataRoot;

use crate::Core;

/// 做完的支线最多留几条。
pub(crate) const KEPT: usize = 20;

/// 订阅时最多认出几条支线再读全：做完的留 [`KEPT`] 条，在跑的是最新的那几条，多留一倍够了。
const CANDIDATES: usize = 2 * KEPT;

/// 订阅时最多读几个会话的第一条：别的群、本机的会话夹在中间，一直往前翻就是把全部会话读一遍。
const LOOKED: usize = 2000;

/// 一条支线这一刻的样子。
#[derive(Debug, Clone, PartialEq)]
struct Branch {
    title: String,
    started: Timestamp,
    ended: Option<Timestamp>,
    state: &'static str,
}

/// 一个场所主线的支线：见过的子会话，是支线的是它的样子，不是的（子代理）是没有。
#[derive(Debug, Default)]
pub(crate) struct Branches {
    seen: BTreeMap<SessionId, Option<Branch>>,
}

impl Branches {
    /// 量一遍主线 `parent` 的支线：会话从新到旧（编号是 UUIDv7），比主线早的不看，每个只读第一条认父会话；认出的支线够
    /// [`CANDIDATES`] 条、或者读了 [`LOOKED`] 个就停，再一条条读全。场所会话不进会话列表的索引（`list.rs` 的 `LOCAL`），
    /// 认父子只能照日志。
    pub(crate) async fn measure(core: &Core, parent: &SessionId) -> Branches {
        let (root, mine) = (core.root.clone(), parent.clone());
        let found = tokio::task::spawn_blocking(move || children(&root, &mine))
            .await
            .unwrap_or_default();
        let mut branches = Branches::default();
        for child in found {
            let branch = read(core, parent, &child).await;
            branches.seen.insert(child, branch);
        }
        branches
    }

    /// 会话 `child` 动了：是 `parent` 的子会话的重读那一条。交回列出来的样子变没变。
    pub(crate) async fn touched(
        &mut self,
        core: &Core,
        parent: &SessionId,
        child: &SessionId,
    ) -> bool {
        if child == parent {
            return false;
        }
        let known = match self.seen.get(child) {
            Some(None) => return false,
            Some(Some(_)) => true,
            None => parent_of(core, child).await.as_ref() == Some(parent),
        };
        if !known {
            return false;
        }
        let before = self.rows();
        let branch = read(core, parent, child).await;
        self.seen.insert(child.clone(), branch);
        self.rows() != before
    }

    /// 列进 `jobs` 的几项：照开始的先后，在跑的都列，做完的只留最近的 [`KEPT`] 条。
    pub(crate) fn rows(&self) -> Vec<Value> {
        let mut branches: Vec<(&SessionId, &Branch)> = self
            .seen
            .iter()
            .filter_map(|(id, branch)| branch.as_ref().map(|branch| (id, branch)))
            .collect();
        branches.sort_by_key(|(id, branch)| (branch.started.unix_millis(), (*id).clone()));
        let finished = branches
            .iter()
            .filter(|(_, branch)| branch.state != "running")
            .count();
        let mut dropped = finished.saturating_sub(KEPT);
        branches
            .into_iter()
            .filter(|(_, branch)| {
                let drop = dropped > 0 && branch.state != "running";
                dropped -= usize::from(drop);
                !drop
            })
            .map(|(id, branch)| {
                let mut row = json!({"job": id, "what": "agent", "fork": true, "session": id,
                    "title": branch.title, "state": branch.state, "started": branch.started});
                if let Some(ended) = branch.ended {
                    row["ended"] = json!(ended);
                }
                row
            })
            .collect()
    }
}

/// 主线 `parent` 的子会话里分叉出来的那几条，从新到旧（在阻塞线程里读，见 [`Branches::measure`]）。
fn children(root: &DataRoot, parent: &SessionId) -> Vec<SessionId> {
    let Ok(ids) = root.sessions() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for id in ids
        .iter()
        .take_while(|id| id.as_str() > parent.as_str())
        .take(LOOKED)
    {
        let Ok(first) = miyu_store::log::first_event(&root.session_dir(id)) else {
            continue;
        };
        if let Body::SessionCreated(created) = &first.body
            && created.parent.as_ref() == Some(parent)
            && created
                .forked
                .as_ref()
                .is_some_and(|forked| forked.within.is_none())
        {
            found.push(id.clone());
            if found.len() >= CANDIDATES {
                break;
            }
        }
    }
    found
}

/// `child` 的父会话：照它日志的第一条读（索引这时可能还没有它那一行）；读不了的没有。
async fn parent_of(core: &Core, child: &SessionId) -> Option<SessionId> {
    let dir = core.root.session_dir(child);
    let first = tokio::task::spawn_blocking(move || miyu_store::log::first_event(&dir))
        .await
        .ok()?
        .ok()?;
    match first.body {
        Body::SessionCreated(created) => created.parent,
        _ => None,
    }
}

/// 读子会话 `child` 的日志：是从 `parent` 分出来的支线（不是 fork 子代理）的交回它的样子，不是的、读不了的没有。
async fn read(core: &Core, parent: &SessionId, child: &SessionId) -> Option<Branch> {
    let dir = core.root.session_dir(child);
    let events = tokio::task::spawn_blocking(move || miyu_store::log::read_events(&dir))
        .await
        .ok()?
        .ok()?;
    let busy = match core.sessions.loaded(child).await {
        Some(handle) => handle.busy(),
        None => false,
    };
    shaped(parent, child, &events, busy)
}

/// 照子会话的事件认支线、算它的样子：`busy` 是它这会儿在不在跑一轮。
fn shaped(parent: &SessionId, child: &SessionId, events: &[Event], busy: bool) -> Option<Branch> {
    let first = events.first()?;
    let Body::SessionCreated(created) = &first.body else {
        return None;
    };
    let forked = created.forked.as_ref()?;
    if forked.from != *parent || forked.within.is_some() {
        return None;
    }
    let mut row = Row::new(child.clone(), first)?;
    for event in &events[1..] {
        row.see(event);
    }
    let last = events.iter().rev().find_map(|event| match &event.body {
        Body::TurnEnded(ended) => Some((event.at, ended.reason.clone())),
        _ => None,
    });
    let (state, ended) = match (busy, last) {
        (false, Some((at, EndReason::Interrupted | EndReason::Aborted))) => ("stopped", Some(at)),
        (false, Some((at, _))) => ("done", Some(at)),
        _ => ("running", None),
    };
    let title = if row.title.is_empty() {
        row.preview
    } else {
        row.title
    };
    Some(Branch {
        title,
        started: first.at,
        ended,
        state,
    })
}

#[cfg(test)]
mod tests;

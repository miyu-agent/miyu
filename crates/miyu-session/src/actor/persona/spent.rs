//! 累计用量照上下文的块摊（施工 CX-3，`session.usage_parts`）：照日志一条条回放有效历史，每次主请求在它看到的那一条
//! （`seen`）记下那一刻的上下文一块块的，连同供应商报的输入；摊、加起来在端点做。只在头问的时候读盘算，不碰回合。
//!
//! - 主请求：`model.called` 里不是辅助请求（没有 `purpose`）、不是压缩的摘要请求，发出去了（有供应商、模型），同用量库的
//!   `main`（子会话的是 `subagent`）。报没报用量都算一次，报了的才摊。
//! - 那一刻用的快照：造会话的那份；换了策略的（`session.policy_changed` 带 `policy`）照内核的规矩，到下一个回合开始才换上。
//! - 每份快照只备一次：system、工具、示范对话照组装一段空的历史估，system 照 CX-1 下的拆法拆。

use std::collections::BTreeMap;

use tokio::sync::oneshot;

use miyu_kernel::event::{Body, Event, ModelCalled};
use miyu_kernel::history::History;
use miyu_kernel::id::{ContentHash, Seq};
use miyu_kernel::session::{Frame, breakdown_in};
use miyu_policy::Snapshot;

use super::Actor;
use super::context::system_of;
use crate::blocking::blocking;
use crate::{Charged, Context, Spent, SystemPart};

/// 一份快照备好的：不随对话变的几块、system 拆开、工具面。
pub(super) struct Prepared {
    pub(super) frame: Frame,
    pub(super) system: Vec<(SystemPart, u64)>,
    pub(super) offered: Vec<String>,
}

impl Actor {
    /// 累计用量照上下文的块摊：读盘、回放在阻塞线程里做，不挡收件箱。没有日志、没有人格快照的（测试里造的）是空的。
    pub(in crate::actor) fn usage_parts(&self, reply: oneshot::Sender<Spent>) {
        let dir = self.store.as_ref().and_then(|store| store.dir());
        let (Some(dir), Some(refresh)) = (dir, self.persona.clone()) else {
            super::super::answer(reply, Spent::default());
            return;
        };
        tokio::spawn(async move {
            let spent = blocking(move || {
                let Ok(events) = miyu_store::log::read_events(&dir) else {
                    return Spent::default();
                };
                replay(events, |hash| {
                    let bytes = refresh.blobs.get(hash).ok()?;
                    let snapshot = Snapshot::from_bytes(&bytes).ok()?;
                    prepare(&snapshot, |total| system_of(&refresh, &snapshot, total))
                })
            })
            .await;
            super::super::answer(reply, spent);
        });
    }
}

/// 备一份快照：造得出策略的才有。system 照 `split` 拆（交进去的是整段估的）。
pub(super) fn prepare(
    snapshot: &Snapshot,
    split: impl FnOnce(u64) -> Vec<(SystemPart, u64)>,
) -> Option<Prepared> {
    let frame = Frame::of(&snapshot.policy().ok()?);
    Some(Prepared {
        system: split(frame.system),
        offered: miyu_policy::sent_names(&snapshot.tools),
        frame,
    })
}

/// 照先后回放 `events`：每次主请求一份。快照照编号用 `prepare` 备，一份只备一次；备不出来的那几次照「其余」算 system、
/// 没有工具面。
pub(super) fn replay(
    events: Vec<Event>,
    mut prepare: impl FnMut(&ContentHash) -> Option<Prepared>,
) -> Spent {
    // 先把主请求照它看到的那一条排好：回放到那一条时记下那一刻。
    let mut waiting: BTreeMap<Seq, Vec<Option<u64>>> = BTreeMap::new();
    let mut spent = Spent::default();
    for event in &events {
        if let Body::ModelCalled(called) = &event.body
            && main(called)
        {
            spent.requests += 1;
            let usage = called.usage.as_ref();
            spent.output += usage.map_or(0, |usage| usage.output);
            let input = usage.map(|usage| usage.uncached + usage.cache_read + usage.cache_write);
            waiting.entry(called.seen).or_default().push(input);
        }
    }
    let mut prepared: BTreeMap<ContentHash, Option<Prepared>> = BTreeMap::new();
    let (mut active, mut pending): (Option<ContentHash>, Option<ContentHash>) = (None, None);
    let mut history = History::default();
    let mut last: Option<Seq> = None;
    for event in events {
        let seq = event.seq;
        match &event.body {
            Body::SessionCreated(created) => active = Some(created.policy.clone()),
            Body::PolicyChanged(changed) if changed.policy.is_some() => {
                pending.clone_from(&changed.policy);
            }
            Body::TurnStarted(_) => {
                if let Some(next) = pending.take() {
                    active = Some(next);
                }
            }
            _ => {}
        }
        history.append(event);
        last = Some(seq);
        if let Some(inputs) = waiting.remove(&seq) {
            charge(
                &mut spent,
                inputs,
                &history,
                active.as_ref(),
                &mut prepared,
                &mut prepare,
            );
        }
    }
    // 看到的那一条日志里没有的（读到一半的日志），照最后那一刻算，总数不丢。
    let rest: Vec<Option<u64>> = waiting.into_values().flatten().collect();
    if !rest.is_empty() && last.is_some() {
        charge(
            &mut spent,
            rest,
            &history,
            active.as_ref(),
            &mut prepared,
            &mut prepare,
        );
    }
    spent
}

/// 这一刻的几次主请求：报了用量的各记一份。
fn charge(
    spent: &mut Spent,
    inputs: Vec<Option<u64>>,
    history: &History,
    active: Option<&ContentHash>,
    prepared: &mut BTreeMap<ContentHash, Option<Prepared>>,
    prepare: &mut impl FnMut(&ContentHash) -> Option<Prepared>,
) {
    let inputs: Vec<u64> = inputs.into_iter().flatten().collect();
    if inputs.is_empty() {
        return;
    }
    let ready = active.and_then(|hash| {
        prepared
            .entry(hash.clone())
            .or_insert_with(|| prepare(hash))
            .as_ref()
    });
    let context = match ready {
        Some(ready) => Context {
            breakdown: breakdown_in(&ready.frame, history, &ready.frame.price),
            system: ready.system.clone(),
            offered: ready.offered.clone(),
        },
        None => {
            let frame = Frame {
                system: 0,
                tools: Vec::new(),
                demos: 0,
                price: miyu_kernel::estimate::Flat { image: 0, file: 0 },
            };
            Context {
                breakdown: breakdown_in(&frame, history, &frame.price),
                system: Vec::new(),
                offered: Vec::new(),
            }
        }
    };
    for input in inputs {
        spent.charged.push(Charged {
            context: context.clone(),
            input,
        });
    }
}

/// 主请求：不是辅助请求、不是压缩的摘要请求，发出去了（同用量库：有供应商、模型才记一行）。
fn main(called: &ModelCalled) -> bool {
    called.purpose.is_none()
        && called.compaction.is_none()
        && called.endpoint.is_some()
        && called.model.is_some()
}

#[cfg(test)]
mod tests;

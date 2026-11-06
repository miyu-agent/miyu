//! 发送前照这个会话自己判前缀变不变（施工 PX-1，`docs/construction/PX-1-前缀变动照会话判.md`）：覆盖、人格、预设那几棵树变了号，
//! 照回合开始那一套（[`super::look`]）把它的快照干算一遍、和现在的比：
//!
//! - 一样的（改的是别的人格、头像、背景、它用不到的覆盖）不用问人，看到的号对到这一次；
//! - 不一样的交回它用着的人格、预设、工具，端点照它们只列真进它前缀的改动。
//!
//! 只看文件带来的变化：工具目录当没换代（照上次拼快照时那一代）、扩展插的字当没变，那两样回合开始照旧换、照旧不问人
//! （施工 OV-1 下只为文件问）。干算在阻塞线程里，不挡收件箱，不建策略、不存 blob；只在号变了的时候端点才来问。
//!
//! 换了人格、预设还没开下一轮的（施工 PX-1补）：快照要等下一个回合才换，干算出来的是新人格的那一份，和还没换下来的旧快照
//! 比永远不一样。人换的那一刻已经放行（压缩过），所以那一刻照同一套干算一遍、记成基准（[`Baseline`]），之后照基准比：
//! 一样的（改的是别的人格的头像）不问、放行到这一次；不一样的才问，交回的是下一个回合真要用的那一份用到的东西。

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::sync::{oneshot, watch};

use miyu_policy::Snapshot;

use super::recast::Wanted;
use super::{Actor, Refresh, Seen, follows_files, look};
use crate::blocking::blocking;
use crate::prefix::{PrefixCheck, Touches};

/// 换人格、换预设那一刻（人已经放行）下一个回合要用的快照（施工 PX-1补）。算好之前来问的等它；算出来是 `None` 的，下一个
/// 回合照旧用现在的快照（读不了、拼不成、换的和现在的一样）。
#[derive(Clone)]
pub(crate) struct Baseline(watch::Receiver<Option<Option<Arc<Snapshot>>>>);

impl Baseline {
    /// 等它算好：算的任务没了的当没有基准。
    async fn settled(mut self) -> Option<Option<Arc<Snapshot>>> {
        self.0
            .wait_for(Option::is_some)
            .await
            .ok()
            .and_then(|settled| settled.clone())
    }
}

impl Actor {
    /// 换人格、换预设刚放行（施工 PX-1补，[`crate::Handle::settle_prefix`]）：照这一刻的文件干算下一个回合要用的快照，记成基准。
    pub(in crate::actor) fn settle_prefix(&mut self) {
        let Some(refresh) = self.persona.clone().filter(follows_files) else {
            return;
        };
        let (values, now, asked, project, wanted) = self.dry_inputs(&refresh);
        let (settle, baseline) = watch::channel(None);
        tokio::spawn(async move {
            let seen = blocking(move || {
                look(
                    &refresh,
                    &values,
                    &now,
                    (asked, project, wanted, true, true),
                )
            })
            .await;
            let settled = match seen {
                Seen::Would(new) => Some(Arc::new(*new)),
                _ => None,
            };
            drop(settle.send(Some(settled)));
        });
        if let Some(refresh) = self.persona.as_mut() {
            refresh.baseline = Some(Baseline(baseline));
        }
    }

    /// 干算要的：这一轮的配置、上次拼快照时那一代、扩展钉着的字、现在的项目、人换的人格预设。
    fn dry_inputs(&self, refresh: &Refresh) -> DryInputs {
        let values = self.config.current().resolved.values();
        let now = refresh
            .seen
            .clone()
            .unwrap_or_else(|| refresh.tools.edition());
        let asked: BTreeMap<String, Option<String>> = refresh
            .snapshot
            .appended
            .iter()
            .map(|(key, text)| (key.clone(), Some(text.clone())))
            .collect();
        let project = self.session.project().map(str::to_string);
        let wanted = Wanted::of(&self.session, &refresh.snapshot, refresh.child);
        (values, now, asked, project, wanted)
    }

    /// 照号 `version` 干算一遍，答给 `reply`。不跟着文件换快照的会话（整理的、fork 子代理）、没有人格快照的（测试里造的）不变。
    pub(in crate::actor) fn check_prefix(&self, version: u64, reply: oneshot::Sender<PrefixCheck>) {
        let Some(refresh) = self.persona.clone().filter(follows_files) else {
            super::super::answer(reply, PrefixCheck::Unchanged);
            return;
        };
        let (values, now, asked, project, wanted) = self.dry_inputs(&refresh);
        let prefix = Arc::clone(&refresh.prefix);
        let baseline = refresh.baseline.clone();
        tokio::spawn(async move {
            let seen = blocking(move || {
                look(
                    &refresh,
                    &values,
                    &now,
                    (asked, project, wanted, true, true),
                )
            })
            .await;
            let found = match seen {
                Seen::Same => {
                    prefix.see(version);
                    PrefixCheck::Unchanged
                }
                // 和换人格那一刻放行过的一样（施工 PX-1补）：放行到这一次，回合开始照旧照文件重拼，拼出来就是它。
                Seen::Would(new) => {
                    let base = match baseline {
                        Some(baseline) => baseline.settled().await.flatten(),
                        None => None,
                    };
                    if base.as_deref() == Some(&*new) {
                        prefix.allow(version);
                        PrefixCheck::Unchanged
                    } else {
                        PrefixCheck::Changed(touches(&new))
                    }
                }
                // 干算不会交这一样。
                Seen::Swapped(..) => PrefixCheck::Unchanged,
                // 读不了、拼不成的回合开始也照旧，前缀不变；号不对上，下次再看。
                Seen::Unreadable(_) | Seen::Kept(_) => PrefixCheck::Unchanged,
            };
            super::super::answer(reply, found);
        });
    }
}

/// 干算要的几样（[`Actor::dry_inputs`]）。
type DryInputs = (
    miyu_config::Values,
    miyu_tool::Edition,
    BTreeMap<String, Option<String>>,
    Option<String>,
    Wanted,
);

/// 快照 `snapshot` 的前缀用到的：人格、预设的编号，工具面上的工具名。
fn touches(snapshot: &Snapshot) -> Touches {
    Touches {
        persona: snapshot.persona.clone(),
        preset: snapshot.preset.as_ref().map(|pin| pin.id.clone()),
        tools: snapshot
            .tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect(),
    }
}

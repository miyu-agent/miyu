//! 压缩触发线换上（施工 CT-1，`docs/blueprint/compaction.md` 第二条第 2 条，`session/actor.md`「换快照」第 5 条）：每个回合
//! 开始，人格、预设那一步看完以后，照会话钉的（内核从日志算的 `Session::pinned_line`）、没钉的照这一轮的配置
//! `compaction.line` 定这一轮的百分数，和现在的快照比：一样的什么都不做；不一样的只换快照里的那一个数
//! （`Snapshot::with_line`），存成 blob，策略交给内核放着，交回新快照的哈希，内核记成换快照（`session.policy_changed` 的
//! `policy`）。这个数不进请求，前缀一个字节不变。载入照日志里记的快照，以前造的快照没有这个数的读成出厂的 85。
//!
//! 冻住的（fork 子代理：快照一直是父会话那一份）不换；裸的（整理会话、顾问会话）本来没有压缩的数，也不换。人格那一步换过的
//! 接着在它换上的那一份上换：内核只认最后交的一份，一个回合只记一条。

use miyu_config::Values;
use miyu_kernel::event::ChangeWhy;
use miyu_kernel::id::ContentHash;

use super::super::Actor;
use crate::TARGET;
use crate::blocking::blocking;
use crate::settings::CompactionSettings;

impl Actor {
    /// 回合开始：人格那一步交回的是 `swapped`（没换的没有）；这一轮的压缩触发线和快照里的不一样的，换上新的那一份，交回它的
    /// 哈希，别的照交回 `swapped`。`values` 是这一轮冻结的配置。
    pub(in crate::actor) async fn reline(
        &mut self,
        values: &Values,
        swapped: Option<ContentHash>,
    ) -> Option<ContentHash> {
        let wanted = CompactionSettings::from(values).line_for(self.session.pinned_line());
        let Some(refresh) = self.persona.as_ref().filter(|refresh| !refresh.frozen) else {
            return swapped;
        };
        let from = refresh.snapshot.line_percent();
        if from.is_none_or(|from| from == wanted) {
            return swapped;
        }
        let new = refresh.snapshot.clone().with_line(Some(wanted));
        let blobs = refresh.blobs.clone();
        let built = blocking(move || -> Result<_, String> {
            let policy = new.policy().map_err(|error| error.to_string())?;
            let hash = blobs
                .put(&new.to_bytes())
                .map_err(|error| error.to_string())?;
            Ok((new, policy, hash))
        })
        .await;
        match built {
            Ok((new, policy, hash)) => {
                tracing::info!(target: TARGET, from, to = wanted, "compaction line changed");
                self.session.stage_policy(policy);
                if let Some(refresh) = self.persona.as_mut() {
                    refresh.snapshot = new;
                }
                Some(hash)
            }
            Err(error) => {
                tracing::warn!(target: TARGET, error = error.as_str(), "compaction line not changed");
                swapped
            }
        }
    }

    /// 内核换上了新的策略快照（施工 CT-1）：压缩线可能变了，给头看的限额照内核这时的重写一次，变了推一条 `model.changed`，
    /// `why` 是 `turn`（同回合开始头看得到的变了）。在写盘以前做，同换上下文上限（`store.rs`）。
    pub(in crate::actor) fn follow_policy(&mut self) {
        let before = self.shown_now();
        self.hand(self.model.limits());
        if self.shown_now() != before {
            self.announce(ChangeWhy::Turn);
        }
    }
}

//! 看守查排队的消息（`docs/designs/02-内核.md` 第六节「排队的消息」）：回合结束时还有排着队的，
//! 下一条就是由最后那条触发的新回合，没有的就不接着开；退回时撤回的，正好是排着队的那几条。还没听到的回报照同一条走
//! （施工 7-2，`watch/reports.rs`）：和排着的消息比，后来的那条接着开；打断的、没人看着的一次性会话不由回报接着开。

use super::*;

impl Watch {
    /// 这一批里的第 `k` 条，照排队的规矩查。
    pub(super) fn queue_check(&mut self, events: &[Event], k: usize) {
        let seed = self.seed;
        let event = &events[k];
        match &event.body {
            Body::MessageUser(_) if event.turn.is_some() => self.queued.push(event.seq),
            // 暂停着没发出去的那一条，她没听到排着的话（施工 6-6 上）：照摘要请求失败一样，回合结束时接着开。
            // 回顾这类辅助请求也不算她听到了（施工 3-8 四补）。
            Body::ModelCalled(called)
                if !called.aside()
                    && called
                        .error
                        .as_ref()
                        .is_none_or(|error| error.class != ErrorClass::CompactionPaused) =>
            {
                let (seen, later): (Vec<Seq>, Vec<Seq>) = self
                    .queued
                    .iter()
                    .partition(|queued| **queued <= called.seen);
                self.heard.extend(seen);
                self.queued = later;
                // 主请求说完了：看到过的算回答过（施工 4-15 补）。
                if called.result == crate::event::CallResult::Ok && called.compaction.is_none() {
                    self.heard.clear();
                }
            }
            Body::MessageWithdrawn(withdrawn) => {
                self.seen_paths.insert("打断后排队的退回");
                // 等停着的那次打断，收尾时照它（又打断了一次的，照后来那次）。
                assert_eq!(
                    self.interrupting.or(self.stopping.queued),
                    Some(Queued::Return),
                    "种子 {seed}：只有退回的打断才撤回"
                );
                assert_eq!(
                    withdrawn.messages, self.queued,
                    "种子 {seed}：撤回的应该正好是排着队的那几条"
                );
                self.queued.clear();
            }
            Body::TurnEnded(ended)
                if matches!(ended.reason, EndReason::Restarted | EndReason::Aborted) =>
            {
                let next = events.get(k + 1).map(|event| &event.body);
                assert!(
                    !matches!(next, Some(Body::TurnStarted(_))),
                    "种子 {seed}：重启、崩了结束的，排着队的也不接着开"
                );
                self.queued.clear();
                self.heard.clear();
                self.keeping = false;
            }
            // 留着的打断（施工 O-6）：排着的不撤回、也不接着开，下一句话开的那一轮里看得到它们。
            Body::TurnEnded(ended) if ended.reason == EndReason::Interrupted && self.keeping => {
                self.keeping = false;
                let next = events.get(k + 1).map(|event| &event.body);
                assert!(
                    !matches!(next, Some(Body::TurnStarted(_))),
                    "种子 {seed}：留着的打断不接着开"
                );
                if !self.queued.is_empty() {
                    self.seen_paths.insert("打断后排队的留着");
                }
                self.queued.clear();
                self.heard.clear();
            }
            Body::TurnEnded(ended) => {
                self.keeping = false;
                let next = events.get(k + 1).map(|event| &event.body);
                // 「马上发」的打断：请求看到过、还没回答的也接着开（施工 4-15 补），排在排着队的前面。
                let send = ended.reason == EndReason::Interrupted
                    && self.interrupting.or(self.stopping.queued) == Some(Queued::Send);
                // 撤那一轮时一起撤的，照旧只是请求一次都没看到过的那几句（加上触发它的那一句，`undone_by` 另算）：看到过的
                // 算上一轮听到过（`history/undo.rs` 的 `left_queued`）。
                let picked = self.queued.clone();
                if send && !self.heard.is_empty() {
                    self.seen_paths.insert("打断后看到过没回答的接着发");
                    let mut heard = std::mem::take(&mut self.heard);
                    heard.append(&mut self.queued);
                    self.queued = heard;
                }
                self.heard.clear();
                let message = self.queued.last().copied();
                let report = self.report_trigger(&ended.reason);
                if report.is_some() {
                    self.seen_paths.insert("回报接着开了一轮");
                }
                if ended.reason == EndReason::Interrupted && !self.reports.pending.is_empty() {
                    self.seen_paths.insert("打断时排着的回报不接着开");
                }
                match message.into_iter().chain(report).max() {
                    Some(last) => {
                        assert!(
                            matches!(next, Some(Body::TurnStarted(started)) if started.trigger == Some(last)),
                            "种子 {seed}：还有排着队的 {last}，回合结束后应该由后来的那条接着开一轮"
                        );
                        // 接着开的那一轮，接过去的是排着的这几句：撤它的时候一起撤。
                        self.undo
                            .picked
                            .insert(TurnId::new(events[k + 1].seq), picked);
                        if message.is_some() {
                            self.seen_paths
                                .insert(if ended.reason == EndReason::Interrupted {
                                    "打断后排队的接着发"
                                } else {
                                    "排队的接着开了一轮"
                                });
                        }
                    }
                    None => assert!(
                        !matches!(next, Some(Body::TurnStarted(_))),
                        "种子 {seed}：没有排着队的，不该接着开"
                    ),
                }
                self.queued.clear();
            }
            _ => {}
        }
    }
}

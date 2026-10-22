//! 看守查排队的消息（`docs/designs/02-内核.md` 第六节「排队的消息」）：回合结束时还有排着队的，
//! 下一条就是由最后那条触发的新回合，没有的就不接着开；退回时撤回的，正好是排着队的那几条。还没听到的回报照同一条走
//! （施工 7-2，`watch/reports.rs`）：和排着的消息比，后来的那条接着开；打断的、没人看着的一次性会话不由回报接着开。不打断退回的
//! （施工 Q-W）：退的正好是要的那一条（不写的是最新的），回合照跑；不在队里的照原因拒。

use super::*;

/// 看守记着的排队。
#[derive(Default)]
pub(super) struct Queue {
    /// 排着队的消息：这一轮里来的，还没被请求看到过。
    pub(super) queued: Vec<Seq>,
    /// 请求看到过、还没得到回答的消息（施工 4-15 补）：请求出错、被打断、被撤掉都还欠着，说完了才算回答过；「马上发」的打断
    /// 以后由它们接着开一轮。
    pub(super) heard: Vec<Seq>,
    /// 正在送进去的那次新的打断，排着队的怎么办。
    pub(super) interrupting: Option<Queued>,
    /// 回合开着时收下的、最近一次打断是「留着」（施工 O-6）：等停着的那一批先清掉了 `stopping`，回合结束时照它认。
    pub(super) keeping: bool,
    /// 正在送进去的那次新的退回（施工 Q-W）：退哪一条，不写的是最新的。
    pub(super) withdrawing: Option<Option<Seq>>,
}

impl Watch {
    /// 送进去之前：新的「退回一条」记下要退哪一条（施工 Q-W）。
    pub(super) fn before_withdraw(&mut self, input: &Input, refused: bool) {
        self.queue.withdrawing = match input {
            Input::Command(command) if !refused && self.fresh(&command.id) => match command.command
            {
                Command::Withdraw { message } => Some(message),
                _ => None,
            },
            _ => None,
        };
    }

    /// 送进去以后：排着的有那一条的接受（退回的事件 [`Watch::queue_check`] 查），没有的照原因拒。
    pub(super) fn after_withdraw(&mut self, actions: &[Action]) {
        let Some(message) = self.queue.withdrawing else {
            return;
        };
        let seed = self.seed;
        let rejected = actions.iter().find_map(|action| match action {
            Action::Reply {
                outcome: Outcome::Rejected { reason, .. },
                ..
            } => Some(*reason),
            _ => None,
        });
        match self.withdrawn_one(message) {
            Some(_) => assert_eq!(rejected, None, "种子 {seed}：排着的退得回"),
            None => {
                self.seen_paths.insert("退回时不在队里");
                let expected = match message {
                    Some(_) => Reason::AlreadyHeard,
                    None => Reason::NothingQueued,
                };
                assert_eq!(rejected, Some(expected), "种子 {seed}：不在队里的照原因拒");
            }
        }
    }

    /// 这次退回该退的那一条：写了的、不写的最新的一条，要在队里、又不在路上的主请求里（内核发主请求时就把队里的挪进听到
    /// 过的，看守等 `model.called` 才挪；摘要请求不带它们）。
    fn withdrawn_one(&self, message: Option<Seq>) -> Option<Seq> {
        let in_flight = self.asking.filter(|seen| !self.summary_seen(*seen));
        let queued: Vec<Seq> = self
            .queue
            .queued
            .iter()
            .copied()
            .filter(|seq| in_flight.is_none_or(|seen| *seq > seen))
            .collect();
        message
            .or(queued.last().copied())
            .filter(|seq| queued.contains(seq))
    }

    /// 这一批里的第 `k` 条，照排队的规矩查。
    pub(super) fn queue_check(&mut self, events: &[Event], k: usize) {
        let seed = self.seed;
        let event = &events[k];
        match &event.body {
            Body::MessageUser(_) if event.turn.is_some() => self.queue.queued.push(event.seq),
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
                    .queue
                    .queued
                    .iter()
                    .partition(|queued| **queued <= called.seen);
                self.queue.heard.extend(seen);
                self.queue.queued = later;
                // 主请求说完了：看到过的算回答过（施工 4-15 补）。
                if called.result == crate::event::CallResult::Ok && called.compaction.is_none() {
                    self.queue.heard.clear();
                }
            }
            Body::MessageWithdrawn(withdrawn) if self.queue.withdrawing.is_some() => {
                self.seen_paths.insert("不打断退回一条");
                let message = self.queue.withdrawing.flatten();
                assert_eq!(
                    withdrawn.messages,
                    Vec::from_iter(self.withdrawn_one(message)),
                    "种子 {seed}：退回的正好是要的那一条"
                );
                self.queue
                    .queued
                    .retain(|seq| !withdrawn.messages.contains(seq));
            }
            Body::MessageWithdrawn(withdrawn) => {
                self.seen_paths.insert("打断后排队的退回");
                // 等停着的那次打断，收尾时照它（又打断了一次的，照后来那次）。
                assert_eq!(
                    self.queue.interrupting.or(self.stopping.queued),
                    Some(Queued::Return),
                    "种子 {seed}：只有退回的打断才撤回"
                );
                assert_eq!(
                    withdrawn.messages, self.queue.queued,
                    "种子 {seed}：撤回的应该正好是排着队的那几条"
                );
                self.queue.queued.clear();
            }
            Body::TurnEnded(ended)
                if matches!(ended.reason, EndReason::Restarted | EndReason::Aborted) =>
            {
                let next = events.get(k + 1).map(|event| &event.body);
                assert!(
                    !matches!(next, Some(Body::TurnStarted(_))),
                    "种子 {seed}：重启、崩了结束的，排着队的也不接着开"
                );
                self.queue.queued.clear();
                self.queue.heard.clear();
                self.queue.keeping = false;
            }
            // 留着的打断（施工 O-6）：排着的不撤回、也不接着开，下一句话开的那一轮里看得到它们。
            Body::TurnEnded(ended)
                if ended.reason == EndReason::Interrupted && self.queue.keeping =>
            {
                self.queue.keeping = false;
                let next = events.get(k + 1).map(|event| &event.body);
                assert!(
                    !matches!(next, Some(Body::TurnStarted(_))),
                    "种子 {seed}：留着的打断不接着开"
                );
                if !self.queue.queued.is_empty() {
                    self.seen_paths.insert("打断后排队的留着");
                }
                self.queue.queued.clear();
                self.queue.heard.clear();
            }
            Body::TurnEnded(ended) => {
                self.queue.keeping = false;
                let next = events.get(k + 1).map(|event| &event.body);
                // 「马上发」的打断：请求看到过、还没回答的也接着开（施工 4-15 补），排在排着队的前面。
                let send = ended.reason == EndReason::Interrupted
                    && self.queue.interrupting.or(self.stopping.queued) == Some(Queued::Send);
                // 撤那一轮时一起撤的，照旧只是请求一次都没看到过的那几句（加上触发它的那一句，`undone_by` 另算）：看到过的
                // 算上一轮听到过（`history/undo.rs` 的 `left_queued`）。
                let picked = self.queue.queued.clone();
                if send && !self.queue.heard.is_empty() {
                    self.seen_paths.insert("打断后看到过没回答的接着发");
                    let mut heard = std::mem::take(&mut self.queue.heard);
                    heard.append(&mut self.queue.queued);
                    self.queue.queued = heard;
                }
                self.queue.heard.clear();
                let message = self.queue.queued.last().copied();
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
                self.queue.queued.clear();
            }
            _ => {}
        }
    }
}

//! 撤销与恢复（`docs/designs/02-内核.md` 第六节「撤销与恢复」）：从选中的那一轮起往后全撤，
//! 她从此看不到；你发下一句之前，能一次一次地恢复。撤掉的拿走、放回，都在有效历史里做
//! （`history/undo.rs`）；能不能撤、能不能恢复，照账本。
//!
//! 文件不改回（施工 U-1，2026-10-15 项目主人定：改回文件交给 git 这类程序，harness 做太危险）：撤销、恢复只动对话，
//! 记下了当场回应。以前的日志里有 `files.restored` 的照样读得进来，不再产生。
//!
//! 撤销能撤掉压缩（`docs/blueprint/kernel/history.md`「撤掉压缩」，施工 6-9）：撤的几轮里有还算数的压缩的，更早的
//! 那一段不在内存里，先叫执行器读回日志，读回来照它重建有效历史再记。恢复不读磁盘：撤掉的连同压缩都放在一边。
//!
//! 撤掉的那几轮派出去、还在跑的任务一起停下（施工 7-8，`agents.md` 第七条第 1 条）：记下撤销的同时出
//! [`Action::StopJobs`]，排在最后；回报记 `undone`、不叫醒她，撤销不等它们。
//!
//! 重做的撤销那一半也走这里（施工 4-7 再补，`redo.rs`）：读回的时候带着 [`Redo`]，撤销记下以后同一批重发、开新的一轮。

use super::action::{Action, Reason};
use super::input::Input;
use super::redo::Redo;
use super::{Session, rejected};
use crate::event::{Body, Event, TurnReverted, TurnUnreverted};
use crate::history::History;
use crate::id::{CommandId, Seq, TurnId};
use crate::origin::By;
use crate::time::Timestamp;

/// 撤掉压缩的撤销正在读回日志（施工 6-9）：是哪个命令、谁发的、撤哪几轮、从第几条读起。
#[derive(Debug)]
pub(super) struct ReadingBack {
    id: CommandId,
    by: By,
    turns: Vec<TurnId>,
    from: Seq,
    /// 读回的时候到的后台命令结束（施工 7-2，`jobs.rs`）：读回来、记了撤销再照先后送进去。
    pub(super) later: Vec<Input>,
    /// 重做的撤销（施工 4-7 再补）：读回来先看最后一轮是不是人的话开的，记了撤销接着重发、开新的一轮。
    redo: Option<Redo>,
}

impl Session {
    /// 从 `turn` 起撤销：记一条 `turn.reverted`，照先后列出它和它以后还没撤掉的每一轮，
    /// `by` 是撤销的人，`cause` 是这个命令；落了盘，回应附上它的序号，头照它找到撤掉的话。
    ///
    /// 有回合在进行的，拒绝，原因码 `turn_running`：头先打断再撤。没有、已经撤掉了的，`unknown_turn`；压缩以前的
    /// 回合照样能撤（施工 6-9）。`turn` 不写的，撤还没撤掉的最后一轮，照账本当场找（施工 4-7 下）；一轮都没有的，
    /// `nothing_to_revert`。撤的几轮里有还算数的压缩的，先出 [`Action::ReadBack`]，读回来再记（[`Session::read_back`]）。
    pub(super) fn revert(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        turn: Option<TurnId>,
    ) -> Vec<Action> {
        if self.turn.is_some() {
            return vec![rejected(id, Reason::TurnRunning)];
        }
        let Some(turn) = turn.or_else(|| self.ledger.last_turn()) else {
            return vec![rejected(id, Reason::NothingToRevert)];
        };
        let Some(turns) = self.ledger.turns_from(turn) else {
            return vec![rejected(id, Reason::UnknownTurn)];
        };
        match self.ledger.read_back_from(turn) {
            Some(from) => self.read_back_first(id, by, turns, from, None),
            None => self.reverted(id, by, at, turns, None),
        }
    }

    /// 撤到还算数的压缩：先出 [`Action::ReadBack`]，读回来再记（[`Session::read_back`]）。重做的带着 `redo`（施工 4-7 再补）。
    pub(super) fn read_back_first(
        &mut self,
        id: CommandId,
        by: By,
        turns: Vec<TurnId>,
        from: Seq,
        redo: Option<Redo>,
    ) -> Vec<Action> {
        self.reading = Some(ReadingBack {
            id,
            by,
            turns,
            from,
            later: Vec::new(),
            redo,
        });
        vec![Action::ReadBack { from }]
    }

    /// 当场记撤销：一条 `turn.reverted`（[`Session::settled`]）。重做的带着 `redo`（施工 4-7 再补）。
    pub(super) fn reverted(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        turns: Vec<TurnId>,
        redo: Option<Redo>,
    ) -> Vec<Action> {
        let stop = self.stop_undone(&turns, &by, &id);
        let body = Body::TurnReverted(TurnReverted { turns });
        let event = self.record(at, by, Some(id.clone()), body);
        let mut actions = self.settled(id, at, event, redo);
        actions.extend(stop);
        actions
    }

    /// 读回的日志来了（施工 6-9）：对得上正在读回的那一次（`from` 一样，事件从第 `from` 条起一条接一条，连到追加过的
    /// 最后一条），有效历史照它重建：留着一切地收这一段，收下 `turn.reverted`，再落到检查点上。新的检查点重读过文件的，
    /// 紧跟着 `Append` 出 `Recall`。对不上的当过时的不理。派出去过的任务照原来那份的：
    /// 读回的那一段以前派的只有它记着（施工 7-2）。读回的时候到的后台命令结束，最后照先后送进去。
    ///
    /// 重做的（施工 4-7 再补）：那一轮的开头、触发的那一条压缩掉了，读回来才看得出它是不是人的话开的、换过以后剩不剩
    /// 东西；不是的拒绝，`not_redoable`，一块都不剩的拒绝，`empty_message`，读回来的都不用，有效历史不换。
    pub(super) fn read_back(
        &mut self,
        at: Timestamp,
        from: Seq,
        events: Vec<Event>,
    ) -> Vec<Action> {
        let expected = self.ledger.next_seq().get().saturating_sub(from.get());
        let fits = self
            .reading
            .as_ref()
            .is_some_and(|reading| reading.from == from)
            && u64::try_from(events.len()).is_ok_and(|n| n == expected)
            && (0..)
                .zip(&events)
                .all(|(k, event)| event.seq.get() == from.get() + k);
        let Some(ReadingBack {
            id,
            by,
            turns,
            later,
            redo,
            ..
        }) = self.reading.take_if(|_| fits)
        else {
            return Vec::new();
        };
        let mut history = History::whole();
        for event in events {
            history.append(event);
        }
        if let Some(reason) = redo
            .as_ref()
            .and_then(|redo| redo.refusal(history.events()))
        {
            let mut actions = vec![rejected(id, reason)];
            for input in later {
                actions.extend(self.handle(input));
            }
            return actions;
        }
        history.jobs_from(&self.history);
        self.history = history.owned_by(self.id.clone());
        let stop = self.stop_undone(&turns, &by, &id);
        let body = Body::TurnReverted(TurnReverted { turns });
        let event = self.record(at, by, Some(id.clone()), body);
        self.history.settle();
        let recall = self.recall();
        let mut actions = self.settled(id, at, event, redo);
        if let Some(recall) = recall {
            actions.insert(1, recall);
        }
        actions.extend(stop);
        for input in later {
            actions.extend(self.handle(input));
        }
        actions
    }

    /// 恢复最近一次撤销：记一条 `turn.unreverted`，列的就是那一次撤掉的那几轮，`by` 是恢复的人，
    /// `cause` 是这个命令。文件不动（施工 U-1）。没有能恢复的（没撤过，或者撤了以后开过回合、压缩过），拒绝，原因码
    /// `nothing_to_unrevert`。那一次撤掉了压缩的，压缩跟着回来，不读磁盘；检查点换了、重读过文件的，紧跟着 `Append` 出
    /// `Recall`（施工 6-9）。记在一边的回报这时开得了，由最后那条接着开一轮，和恢复那一条同一批（施工 7-2，`jobs.rs`）。
    pub(super) fn unrevert(&mut self, id: CommandId, by: By, at: Timestamp) -> Vec<Action> {
        let Some(turns) = self.ledger.last_reverted().map(<[TurnId]>::to_vec) else {
            return vec![rejected(id, Reason::NothingToUnrevert)];
        };
        let checkpoint = self.checkpoint_seq();
        let body = Body::TurnUnreverted(TurnUnreverted { turns });
        let event = self.record(at, by, Some(id.clone()), body);
        let recall = match self.checkpoint_seq() == checkpoint {
            true => None,
            false => self.recall(),
        };
        let mut actions = self.settled(id, at, event, None);
        if let Some(Action::Append(events)) = actions.first_mut() {
            events.extend(self.wake_deferred(at));
        }
        if let Some(recall) = recall {
            actions.insert(1, recall);
        }
        actions
    }

    /// 有效历史现在的检查点是第几条；没有检查点就没有。
    fn checkpoint_seq(&self) -> Option<Seq> {
        self.history.checkpoint().map(|checkpoint| checkpoint.seq)
    }

    /// 撤销、恢复记下了：等它落了盘就回应，第一个动作总是追加那一条。重做的（施工 4-7 再补）：重发的几句、新的一轮的开头
    /// 和撤销同一批，回应附上撤销和重发的几句。
    fn settled(
        &mut self,
        id: CommandId,
        at: Timestamp,
        event: Event,
        redo: Option<Redo>,
    ) -> Vec<Action> {
        let mut accepted = vec![event.seq];
        let mut events = vec![event];
        if let Some(redo) = redo {
            let resent = self.resend(at, &id, redo);
            accepted.extend(said(&resent));
            events.extend(resent);
        }
        self.accept(id, accepted);
        vec![Action::Append(events)]
    }
}

/// 重发的那一批里人的话（`message.user`）的序号，照先后：重做的回应附上它们，新的一轮的开头不附（施工 4-7 再补）。
fn said(events: &[Event]) -> Vec<Seq> {
    events
        .iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.seq)
        .collect()
}

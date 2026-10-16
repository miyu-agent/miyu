//! 一条线这一轮的样子（施工 O-36 从 `projection.rs` 挪出来，`onebot.md` 第一条「群里怎么叫她」第 2 条）：主线、每条支线各一份，
//! 纯逻辑。在跑的那一轮（回合编号、回的人、她回的那一条、在回的几条）、并进来的几条（核心接着开的一轮照它找回触发）、这一轮
//! 入队了的她的话（出站链去重）。两条线的回合编号各数各的、会撞上（支线照子会话自己的序号数），所以分开放，不按回合编号混在一起。

use std::collections::HashMap;

use miyu_client::ids::{ExternalId, Seq};
use miyu_client::time::Timestamp;

use super::{Projection, Turn, add};

/// 一条线：主线或者一条支线。
#[derive(Debug, Default)]
pub(super) struct Track {
    /// 在跑的那一轮。
    pub(super) running: Option<Running>,
    /// 并进一轮的那几条：`turn.joined` 在这条线里的序号 → 它的 `triggers`（已经是主线序号，O-23 下）。核心接着开的一轮照它找回
    /// 触发的人。
    pub(super) joined: HashMap<u64, Vec<Seq>>,
    /// 最近一轮发出去的（O-25 上）。
    round: Round,
}

/// 一条线在跑的那一轮。
#[derive(Debug, Clone)]
pub(super) struct Running {
    /// 回合编号（开始的那一条在这条线里的序号）。
    pub(super) turn: u64,
    /// 这一轮回的人：`triggers`、`turn.joined.triggers` 的发的人，不重。
    pub(super) to: Vec<ExternalId>,
    /// 她这时在回的那一条的主线序号（O-25 上）：`triggers` 的最后一条，并进来的换成 `turn.joined.triggers` 的最后一条（和核心记
    /// `tool.call` 的 `by` 一个取法）；没有触发的是空的。
    pub(super) aim: Option<u64>,
    /// 这一轮在回的几条的主线序号，照先后、不重（O-36：开轮时给别的线带「另一条线正在回」照它）。
    pub(super) msgs: Vec<u64>,
}

/// 一轮发出去的（O-25 上）：去重只看这一回合。
#[derive(Debug, Default)]
struct Round {
    /// 回合编号：换了回合就清。
    turn: u64,
    /// 这一轮入队了的她的话的正文，照先后，一样的只记一次（O-25 中）。
    texts: Vec<String>,
}

impl Track {
    /// 开一轮的触发（主线序号）：`triggers` 是空的、`trigger` 指向这条线里一条 `turn.joined` 的（核心接着开的一轮，O-23 下），照
    /// 记下的并进来的找回；别的照 `triggers`。
    pub(super) fn triggers(&self, triggers: Vec<Seq>, trigger: Option<Seq>) -> Vec<Seq> {
        match (triggers.is_empty(), trigger) {
            (true, Some(trigger)) => self.joined.get(&trigger.get()).cloned().unwrap_or_default(),
            _ => triggers,
        }
    }

    /// 回合编号是 `turn` 的那一轮入队了她的一段 `text`（O-25 中）：桥入队记成了先算上，日志推来的同一段照正文认，这一轮已经有
    /// 一样的不再加（「施工时定的」第 112 条）；换了回合的从这一轮重新记；晚到的、更早一轮的不算。
    pub(super) fn queued(&mut self, turn: u64, text: &str) {
        if turn < self.round.turn {
            return;
        }
        if turn > self.round.turn {
            self.round = Round {
                turn,
                texts: Vec::new(),
            };
        }
        if !self.round.texts.iter().any(|one| one == text) {
            self.round.texts.push(text.to_string());
        }
    }

    /// 回合编号是 `turn` 的那一轮入队了的正文，照先后；不是最近一轮的是空的。
    pub(super) fn sent(&self, turn: u64) -> Vec<String> {
        if self.round.turn == turn {
            self.round.texts.clone()
        } else {
            Vec::new()
        }
    }
}

impl Projection {
    /// 线 `line`（主线或者支线，O-36）回合编号是 `turn` 的那一轮入队了她的一段 `text`（O-25 中，[`Track::queued`]）；已经去掉
    /// 了的支线不记。
    pub(in crate::core::route) fn queued(&mut self, line: &str, turn: u64, text: &str) {
        if let Some(track) = self.track_mut(line) {
            track.queued(turn, text);
        }
    }

    /// 线 `line` 开了回合编号是 `turn` 的一轮（O-36 起主线、支线同一个算法），触发是 `triggers`（主线序号），开始的时刻是 `at`：
    /// 记进开过的回合（进站链的 `turns`），那条线这一轮回的人、她回的那一条照它，收了判过要回的记下是哪一轮。
    pub(super) fn started(
        &mut self,
        line: &str,
        triggers: &[Seq],
        turn: Option<u64>,
        at: Timestamp,
    ) {
        let by = self.speakers(triggers);
        let mut to = Vec::new();
        add(&mut to, by.iter().map(|speaker| &speaker.id));
        self.turns.push(Turn { at, by });
        let running = turn.map(|turn| Running {
            turn,
            to,
            aim: triggers.last().map(|seq| seq.get()),
            msgs: triggers.iter().map(|seq| seq.get()).collect(),
        });
        if let Some(track) = self.track_mut(line) {
            track.running = running;
        }
        self.taken(triggers, line, turn);
        self.prune();
    }

    /// 线 `line` 里序号是 `seq` 的 `turn.joined`：`triggers`（主线序号）并进回合编号是 `turn` 的那一轮。
    pub(super) fn joined(&mut self, line: &str, seq: u64, triggers: &[Seq], turn: Option<u64>) {
        let by = self.speakers(triggers);
        if let Some(track) = self.track_mut(line) {
            track.joined.insert(seq, triggers.to_vec());
            if let Some(running) = track.running.as_mut()
                && Some(running.turn) == turn
            {
                add(&mut running.to, by.iter().map(|speaker| &speaker.id));
                if let Some(last) = triggers.last() {
                    running.aim = Some(last.get());
                }
                for one in triggers {
                    if !running.msgs.contains(&one.get()) {
                        running.msgs.push(one.get());
                    }
                }
            }
        }
        self.taken(triggers, line, turn);
    }

    /// 线 `line` 回合编号是 `turn` 的那一轮完了：判过要回、那一轮收了的回完了。
    pub(super) fn ended(&mut self, line: &str, turn: Option<u64>) {
        if let Some(track) = self.track_mut(line) {
            track.running = None;
        }
        let key = turn.map(|turn| (line.to_string(), turn));
        self.committed
            .retain(|committed| committed.turn.is_none() || committed.turn != key);
        self.prune();
    }

    /// 线 `line` 的那一份：主线的，或者还在的那条支线的。
    pub(super) fn track(&self, line: &str) -> Option<&Track> {
        if line == self.session {
            return Some(&self.main);
        }
        self.lanes
            .iter()
            .find(|lane| lane.session == line)
            .map(|lane| &lane.track)
    }

    /// 同 [`Projection::track`]，改得了。
    pub(super) fn track_mut(&mut self, line: &str) -> Option<&mut Track> {
        if line == self.session {
            return Some(&mut self.main);
        }
        self.lanes
            .iter_mut()
            .find(|lane| lane.session == line)
            .map(|lane| &mut lane.track)
    }

    /// 那几条（`triggers`，主线序号）进了线 `line` 回合编号是 `turn` 的那一轮：收了它们的判过要回的，记下是哪条线的哪一轮。
    fn taken(&mut self, triggers: &[Seq], line: &str, turn: Option<u64>) {
        for committed in &mut self.committed {
            let pending = &committed.pending;
            let mine = |seq: &Seq| *seq == pending.msg || pending.absorbed.contains(seq);
            if committed.turn.is_none() && triggers.iter().any(mine) {
                committed.turn = turn.map(|turn| (line.to_string(), turn));
            }
        }
    }
}

//! 回合里的几步和撤销、恢复、回报（施工 V-41 中，`undo_log.rs` 放不下了）：她的回复和请求、工具结果和它的效果、结束一轮、撤销、
//! 恢复、重做、撤回排着的话、压缩、后台命令和子代理的回报。

use super::{Dice, Job, KERNEL, MODEL, PATHS, PERSON, UndoLog};

impl UndoLog {
    /// 一轮里的一步。
    pub(super) fn inside(&mut self, dice: &mut Dice, turn: u64) {
        if !self.pending.is_empty() && !dice.odds(4) {
            self.result(dice, turn);
            return;
        }
        let seq = self.next();
        let floor = self.last_reply.unwrap_or(self.first);
        let seen = match dice.below(3) {
            0 => floor.max(turn).min(seq - 1),
            _ => seq - 1,
        };
        match dice.below(6) {
            0..=2 => {
                let calls = match dice.below(3) {
                    0 => 0,
                    1 => 1,
                    _ => 1 + dice.below(3),
                };
                let mut blocks = vec![r#"{"type":"text","text":"好"}"#.to_string()];
                for k in 1..=calls {
                    let id = format!("call_{seq}_{k}");
                    blocks.push(format!(
                        r#"{{"type":"tool_call","call_id":"{id}","name":"read","args":"{{}}"}}"#
                    ));
                    self.pending.push(id);
                }
                self.line(
                    "message.assistant",
                    Some(turn),
                    MODEL,
                    None,
                    &format!(r#"{{"blocks":[{}],"seen":{seen}}}"#, blocks.join(",")),
                );
                self.last_reply = Some(seq);
                self.queued.retain(|queued| *queued > seen);
            }
            3 => {
                self.line(
                    "model.called",
                    Some(turn),
                    KERNEL,
                    None,
                    &format!(r#"{{"seen":{seen},"messages":1,"result":"ok"}}"#),
                );
                self.queued.retain(|queued| *queued > seen);
            }
            4 => self.line("ext.note", Some(turn), KERNEL, None, r#"{"x":[1]}"#),
            _ => self.say(dice),
        }
    }

    /// 一个还没有结果的调用的结果，带着效果。
    pub(super) fn result(&mut self, dice: &mut Dice, turn: u64) {
        let k = dice.below(self.pending.len() as u64) as usize;
        let call = self.pending.remove(k);
        // 同一条里派出去的子代理还不能留言（账本先查留言）：只挑这一条以前的。
        let before = self.jobs.len() as u64;
        let mut effects = Vec::new();
        for _ in 0..dice.below(4) {
            effects.push(self.effect(dice, before));
        }
        let effects = match effects.is_empty() {
            true => String::new(),
            false => format!(r#","effects":[{}]"#, effects.join(",")),
        };
        self.line(
            "tool.result",
            Some(turn),
            &format!(r#"{{"kind":"tool","call_id":"{call}"}}"#),
            None,
            &format!(
                r#"{{"call_id":"{call}","status":"ok","blocks":[{{"type":"text","text":"x"}}]{effects},"duration_ms":3}}"#
            ),
        );
    }

    /// 一样效果：读、改、移进回收站的文件，派出去的任务，给子代理留的言（第 `before` 个以前派的），写的待办。
    pub(super) fn effect(&mut self, dice: &mut Dice, before: u64) -> String {
        let path = PATHS[dice.below(PATHS.len() as u64) as usize];
        let hash = |n: u64| format!("sha256:{n:064x}");
        match dice.below(9) {
            0..=1 => format!(
                r#"{{"kind":"file.read","path":"{path}","hash":"{}"}}"#,
                hash(dice.below(5))
            ),
            2 => format!(
                r#"{{"kind":"file.read","path":"{path}","lines":[1,20],"hash":"{}"}}"#,
                hash(dice.below(5))
            ),
            3 => format!(
                r#"{{"kind":"file.changed","path":"{path}","before":"{}","after":"{}","before_text":"a","after_text":"b"}}"#,
                hash(dice.below(5)),
                hash(dice.below(5))
            ),
            4 => format!(
                r#"{{"kind":"file.changed","path":"{path}","before":null,"after":"{}"}}"#,
                hash(dice.below(5))
            ),
            5 => format!(r#"{{"kind":"file.trashed","path":"{path}","trash":"t1"}}"#),
            6 => {
                let number = self.jobs.len() as u64 + 1;
                self.jobs.insert(number, Job::Command { ended: false });
                format!(
                    r#"{{"kind":"job.started","job":"j{number}","what":"command","title":"跑 {number}"}}"#
                )
            }
            7 => {
                let number = self.jobs.len() as u64 + 1;
                self.jobs.insert(
                    number,
                    Job::Agent {
                        session: number,
                        stopped: false,
                    },
                );
                let foreground = if dice.odds(4) {
                    r#","foreground":true"#
                } else {
                    ""
                };
                format!(
                    r#"{{"kind":"job.started","job":"j{number}","what":"agent","title":"查 {number}","session":"{}"{foreground}}}"#,
                    agent(number)
                )
            }
            _ => {
                let agents: Vec<u64> = self
                    .jobs
                    .iter()
                    .filter(|(number, job)| **number <= before && matches!(job, Job::Agent { .. }))
                    .map(|(number, _)| *number)
                    .collect();
                match agents.is_empty() {
                    true => r#"{"kind":"todo.written","todos":[]}"#.to_string(),
                    false => format!(
                        r#"{{"kind":"job.messaged","job":"j{}"}}"#,
                        agents[dice.below(agents.len() as u64) as usize]
                    ),
                }
            }
        }
    }

    /// 结束这一轮：先给还没有结果的调用补上结果。
    pub(super) fn end(&mut self, dice: &mut Dice, turn: u64) {
        while !self.pending.is_empty() {
            self.result(dice, turn);
        }
        self.line(
            "turn.ended",
            Some(turn),
            KERNEL,
            None,
            r#"{"reason":"completed"}"#,
        );
        self.open = None;
        self.queued.clear();
        // 排着的话接着开下一轮：两条挨着。
        if dice.odds(4) {
            self.start(dice, None);
        }
    }

    /// 撤几轮：最近一轮、最近几轮、很早的一轮起；分叉出来的偶尔从分叉点以前的一轮起。
    pub(super) fn revert(&mut self, dice: &mut Dice) {
        let mut turns = if !self.base.is_empty() && dice.odds(8) {
            let from = dice.below(self.base.len() as u64) as usize;
            self.reached_base = true;
            let mut turns = self.base.split_off(from);
            turns.append(&mut self.turns);
            turns
        } else if self.turns.is_empty() {
            return;
        } else {
            let from = match dice.below(6) {
                0 => dice.below(self.turns.len() as u64) as usize,
                1 => self.turns.len().saturating_sub(3),
                _ => self.turns.len() - 1,
            };
            self.turns.split_off(from)
        };
        turns.sort_unstable();
        let first = turns[0];
        let kept = self.compactions.partition_point(|(turn, _)| *turn < first);
        let compactions = self.compactions.split_off(kept);
        self.undos.push(self.next());
        self.line(
            "turn.reverted",
            None,
            PERSON,
            None,
            &format!(r#"{{"turns":{turns:?}}}"#),
        );
        self.undone.push((turns, compactions));
    }

    /// 恢复最近一次撤销。
    pub(super) fn unrevert(&mut self) {
        let Some((turns, compactions)) = self.undone.pop() else {
            return;
        };
        self.undos.push(self.next());
        self.line(
            "turn.unreverted",
            None,
            PERSON,
            None,
            &format!(r#"{{"turns":{turns:?}}}"#),
        );
        self.turns.extend(&turns);
        self.turns.sort_unstable();
        self.compactions.extend(compactions);
    }

    /// 重做（编辑重发）：撤掉最后一轮，开它的人话同一个命令重发（字可能换了），再开一轮。
    pub(super) fn redo(&mut self, dice: &mut Dice) {
        let Some(&turn) = self.turns.last() else {
            return;
        };
        let cause = self.said.last().map_or(0, |said| said.1);
        self.turns.pop();
        let kept = self.compactions.partition_point(|(at, _)| *at < turn);
        let compactions = self.compactions.split_off(kept);
        self.undos.push(self.next());
        self.line(
            "turn.reverted",
            None,
            PERSON,
            None,
            &format!(r#"{{"turns":[{turn}]}}"#),
        );
        self.undone.push((vec![turn], compactions));
        let seq = self.next();
        let body = UndoLog::words(dice);
        self.line("message.user", None, PERSON, Some(cause), &body);
        self.said.push((seq, cause));
        self.start(dice, Some(seq));
    }

    /// 撤回排着的话。
    pub(super) fn withdraw(&mut self, dice: &mut Dice, turn: u64) {
        let picked: Vec<u64> = self
            .queued
            .iter()
            .copied()
            .filter(|_| dice.odds(2))
            .collect();
        if picked.is_empty() {
            return;
        }
        self.queued.retain(|queued| !picked.contains(queued));
        self.line(
            "message.withdrawn",
            Some(turn),
            KERNEL,
            None,
            &format!(r#"{{"messages":{picked:?}}}"#),
        );
    }

    /// 压缩、清空：替代到的不早于还算数的最近一次。
    pub(super) fn compact(&mut self, dice: &mut Dice, turn: u64) {
        let seq = self.next();
        let floor = self
            .compactions
            .last()
            .map_or(self.first, |(_, upto)| *upto);
        let upto = floor + dice.below(seq - floor);
        let body = match dice.below(4) {
            0 => format!(r#"{{"upto":{upto},"summary":"","trigger":"clear"}}"#),
            1 => format!(r#"{{"upto":{upto},"summary":"s","trigger":"manual"}}"#),
            2 => format!(r#"{{"upto":{upto},"summary":"s","trigger":"overflow"}}"#),
            _ => format!(r#"{{"upto":{upto},"summary":"s"}}"#),
        };
        self.line("context.compacted", Some(turn), KERNEL, None, &body);
        self.compactions.push((turn, upto));
        self.undone.clear();
    }

    /// 后台命令、子代理报上来：对得上一个还会报的；没有的不报。
    pub(super) fn report(&mut self, dice: &mut Dice) {
        let live: Vec<u64> = self
            .jobs
            .iter()
            .filter(|(_, job)| match job {
                Job::Command { ended } => !ended,
                Job::Agent { stopped, .. } => !stopped,
            })
            .map(|(number, _)| *number)
            .collect();
        if live.is_empty() {
            return;
        }
        let number = live[dice.below(live.len() as u64) as usize];
        let seq = self.next();
        let job = format!("j{number}");
        let report = 9000 + seq;
        match self.jobs.get_mut(&number) {
            Some(Job::Command { ended }) => {
                *ended = true;
                self.line(
                    "job.reported",
                    None,
                    KERNEL,
                    Some(report),
                    &format!(
                        r#"{{"job":"{job}","reason":"exited","exit_code":0,"duration_ms":5,"output":"sha256:3245795a51c1f9e64adb119c6b258a9079a4fdc7f1a15677f1d15d09910bef9c","chars":4}}"#
                    ),
                );
            }
            Some(Job::Agent { session, stopped }) => {
                let reason =
                    ["done", "done", "stopped", "undone", "aborted"][dice.below(5) as usize];
                *stopped = matches!(reason, "stopped" | "undone");
                let session = agent(*session);
                self.line(
                    "child.reported",
                    None,
                    &format!(r#"{{"kind":"session","id":"{session}"}}"#),
                    Some(report),
                    &format!(
                        r#"{{"job":"{job}","session":"{session}","reason":"{reason}","text":"好了"}}"#
                    ),
                );
            }
            None => return,
        }
        self.reports.push(seq);
    }
}

/// 第 `number` 个子代理的子会话编号。
fn agent(number: u64) -> String {
    format!("01900000-0000-7000-8000-{number:012x}")
}

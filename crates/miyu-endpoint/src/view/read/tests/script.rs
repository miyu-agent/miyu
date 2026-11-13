//! 照种子造会话日志（施工 V-41 上的对照测试用）：人说话（闲着说的、一轮里排着的，正文里带转义的控制字符、带 `"job.started"`
//! 字样的）、开回合（照刚才一句、照很早以前一句、照一条回报、不带触发、触发写成日志里没有的序号）、她的回复和请求、工具结果
//! （派后台命令、派子代理、给子代理留言，结果原文里带转义的控制字符、带 `"job.` 字样）、脚本里调的工具（挂在一次调用、挂在一个
//! 后台任务下面）、回报、子会话回报（一轮里到的、闲着到的、照它开下一轮的）、撤回排着的话、结束一轮（做完、打断）、撤销（最近
//! 一轮、最近几轮、压缩以前的一轮）、恢复、重做（撤了接着重说一遍）、压缩、改标题、斜杠命令、不认识的种类。
//!
//! 内核写不出来、但读得成的样子也造一些：种类写在 `body` 后面的、外壳字段换了先后的（翻页认不出样子，照解）、效果的种类写成
//! 转义的（`"job\u002estarted"`）。子会话的第一条带 `parent`；分叉出来的会话从中间的序号起，回合的触发、调用编号可能指到
//! 它前面（父会话里的）。一行一条，序号接着往下数。

use serde_json::{Value, json};

use miyu_kernel::event::Event;

/// 照种子走的随机数（xorshift，同 V-37 下、V-39 上的对照测试）。
pub(super) struct Dice(pub(super) u64);

impl Dice {
    /// `0..n` 里的一个。
    pub(super) fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }

    /// `n` 回里有一回。
    pub(super) fn odds(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }

    /// 挑一个。
    fn pick<'a, T>(&mut self, from: &'a [T]) -> &'a T {
        &from[usize::try_from(self.below(from.len() as u64)).unwrap_or(0)]
    }
}

const KERNEL: &str = r#"{"kind":"kernel"}"#;
const PERSON: &str = r#"{"kind":"person"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
const POLICY: &str = "sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894";
const TOOLS: [&str; 6] = ["shell", "read", "edit", "subagent", "send_message", "grep"];
const WORDS: [&str; 6] = [
    "看看 src",
    "跑一下测试 \u{1b}[31mred\u{1b}[0m",
    "说一句 \"job.started\" 试试",
    "整理一下",
    "line\nbreak",
    "好",
];

/// 造着的一段日志。
pub(super) struct Script {
    /// 写好的行。
    pub(super) lines: Vec<String>,
    /// 第一条的序号：分叉出来的会话不从 1 起。
    pub(super) first: u64,
    /// 正在跑的那一轮（`turn.started` 的序号）。
    open: Option<u64>,
    /// 开过的、没撤掉的回合。
    turns: Vec<u64>,
    /// 刚撤的那几轮，能恢复。
    undone: Vec<Vec<u64>>,
    /// 人说的话：序号。
    said: Vec<u64>,
    /// 排着的话。
    queued: Vec<u64>,
    /// 回报：序号。
    reports: Vec<u64>,
    /// 任务编号用到第几个。
    jobs: u64,
    /// 在跑的任务：编号、是不是子代理、派它的调用编号。
    running: Vec<(String, bool, String)>,
    /// 派过的子代理：编号。
    agents: Vec<String>,
    /// 用过的调用编号。
    calls: Vec<String>,
    /// 命令编号用到第几个。
    command: u64,
    /// 毫秒。
    now: u64,
}

impl Script {
    /// 第一条是 `session.created`：`fork` 不是 1 的从它起（分叉出来的），`child` 的带 `parent`（子会话）。
    pub(super) fn new(fork: u64, child: bool) -> Script {
        let mut script = Script {
            lines: Vec::new(),
            first: fork,
            open: None,
            turns: Vec::new(),
            undone: Vec::new(),
            said: Vec::new(),
            queued: Vec::new(),
            reports: Vec::new(),
            jobs: 0,
            running: Vec::new(),
            agents: Vec::new(),
            calls: Vec::new(),
            command: 0,
            now: 0,
        };
        let mut body = json!({"venue": "local", "policy": POLICY, "permission": {"level": "workspace", "read_only": false}});
        if child {
            body["parent"] = json!(PARENT);
            body["depth"] = json!(1);
        }
        script.push("session.created", None, PERSON, &body, Shape::Written);
        script
    }

    /// 下一条的序号。
    fn next(&self) -> u64 {
        self.first + self.lines.len() as u64
    }

    /// 写一条：`turn` 是它在哪一轮，`shape` 是写成什么样子。交回它的序号。
    fn push(&mut self, kind: &str, turn: Option<u64>, by: &str, body: &Value, shape: Shape) -> u64 {
        let seq = self.next();
        self.now += 137 + seq % 997;
        let ms = self.now;
        let at = format!(
            "2026-11-03T{:02}:{:02}:{:02}.{:03}Z",
            (ms / 3_600_000) % 24,
            (ms / 60_000) % 60,
            (ms / 1000) % 60,
            ms % 1000
        );
        self.command += u64::from(by == PERSON);
        let cause = format!("cmd-{}", self.command);
        let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
        let body = body.to_string();
        let line = match shape {
            Shape::Written => format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"{kind}"{turn},"by":{by},"cause":"{cause}","body":{body}}}"#
            ),
            Shape::KindLast => format!(
                r#"{{"seq":{seq},"at":"{at}"{turn},"by":{by},"cause":"{cause}","body":{body},"kind":"{kind}"}}"#
            ),
            Shape::AtFirst => format!(
                r#"{{"at":"{at}","seq":{seq},"kind":"{kind}"{turn},"by":{by},"body":{body}}}"#
            ),
            Shape::Escaped => format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"{kind}"{turn},"by":{by},"cause":"{cause}","body":{}}}"#,
                body.replace(r#""kind":"job.started""#, r#""kind":"job\u002estarted""#)
                    .replace(r#""kind":"job.messaged""#, r#""kind":"job\u002emessaged""#)
            ),
        };
        let event = Event::from_line(&line).unwrap_or_else(|error| panic!("{line}: {error}"));
        assert_eq!(event.seq.get(), seq);
        self.lines.push(line);
        seq
    }

    /// 照种子走 `steps` 步。
    pub(super) fn play(&mut self, dice: &mut Dice, steps: u64) {
        for _ in 0..steps {
            let shape = match dice.below(40) {
                0 => Shape::KindLast,
                1 => Shape::AtFirst,
                2 => Shape::Escaped,
                _ => Shape::Written,
            };
            match (self.open, dice.below(22)) {
                (_, 20 | 21) => self.nested(dice, shape),
                (_, 0) => self.say(dice, shape),
                (None, 1..=8) => self.start(dice, shape),
                (None, 9) => self.revert(dice, shape),
                (None, 10) => self.unrevert(shape),
                (None, 11) => self.compact(shape),
                (None, 12) => self.redo(dice, shape),
                (_, 13) => self.report(dice, shape),
                (_, 14) => self.misc(dice, shape),
                (Some(turn), 15) => self.end(dice, turn, shape),
                (Some(turn), 16) => self.withdraw(turn, shape),
                (Some(turn), _) => self.step(dice, turn, shape),
                (None, _) => self.say(dice, shape),
            }
        }
    }

    /// 人说一句：一轮在跑的排着。
    fn say(&mut self, dice: &mut Dice, shape: Shape) {
        let text = *dice.pick(&WORDS);
        let body = json!({"blocks": [{"type": "text", "text": text}]});
        let seq = self.push("message.user", self.open, PERSON, &body, shape);
        self.said.push(seq);
        if self.open.is_some() {
            self.queued.push(seq);
        }
    }

    /// 开一轮。
    fn start(&mut self, dice: &mut Dice, shape: Shape) {
        let trigger = match dice.below(8) {
            0 => None,
            1 => self.said.first().copied(),
            2 => self.reports.last().copied(),
            3 => Some(self.first.saturating_sub(1).max(1)),
            4 => Some(self.next() + 5),
            _ => {
                if self.said.last().is_none() || dice.odds(3) {
                    self.say(dice, Shape::Written);
                }
                self.said.last().copied()
            }
        };
        let body = trigger.map_or(json!({}), |trigger| json!({"trigger": trigger}));
        let seq = self.next();
        self.push("turn.started", Some(seq), KERNEL, &body, shape);
        self.open = Some(seq);
        self.turns.push(seq);
        self.undone.clear();
    }

    /// 一轮里的一步：一条回复带几次调用，请求，每次调用的结果。
    fn step(&mut self, dice: &mut Dice, turn: u64, shape: Shape) {
        let seq = self.next();
        let mut blocks = vec![json!({"type": "text", "text": *dice.pick(&WORDS)})];
        let count = dice.below(4);
        let mut calls = Vec::new();
        for index in 1..=count {
            let call = format!("call_{seq}_{index}");
            let name = *dice.pick(&TOOLS);
            let args = match name {
                "send_message" => {
                    json!({"to": self.agents.last().cloned().unwrap_or_else(|| "j1".to_string()), "text": "接着"})
                }
                "subagent" => json!({"description": "查一下", "prompt": "查"}),
                _ => json!({"command": format!("ls {index}"), "background": dice.odds(2)}),
            };
            blocks.push(json!({"type": "tool_call", "call_id": call, "name": name, "args": args.to_string()}));
            calls.push((call, name));
        }
        let seen = self.next() - 1;
        self.push(
            "message.assistant",
            Some(turn),
            MODEL,
            &json!({"blocks": blocks, "seen": seen}),
            shape,
        );
        self.push(
            "model.called",
            Some(turn),
            KERNEL,
            &json!({"seen": seen, "messages": 3, "result": "ok", "duration_ms": 20}),
            Shape::Written,
        );
        for (call, name) in calls {
            self.result(dice, turn, &call, name, shape);
            self.calls.push(call);
        }
        if dice.odds(4) {
            self.nested(dice, Shape::Written);
        }
    }

    /// 一次调用的结果：派任务、留言的带效果。
    fn result(&mut self, dice: &mut Dice, turn: u64, call: &str, name: &str, shape: Shape) {
        let mut effects = Vec::new();
        match name {
            "subagent" => {
                self.jobs += 1;
                let job = format!("j{}", self.jobs);
                effects.push(json!({"kind": "job.started", "job": job, "what": "agent", "title": format!("子代理 {job}"), "session": PARENT}));
                self.running.push((job.clone(), true, call.to_string()));
                self.agents.push(job);
            }
            "send_message" if !self.agents.is_empty() => {
                let job = dice.pick(&self.agents).clone();
                effects.push(json!({"kind": "job.messaged", "job": job}));
            }
            "shell" if dice.odds(2) => {
                self.jobs += 1;
                let job = format!("j{}", self.jobs);
                effects.push(json!({"kind": "job.started", "job": job, "what": "command", "title": format!("后台 {job}")}));
                self.running.push((job, false, call.to_string()));
            }
            _ => {}
        }
        let text = match dice.below(4) {
            0 => "out \u{1b}[1mbold\u{1b}[0m".to_string(),
            1 => "\"job.started\" 只是字".to_string(),
            _ => format!("ok {call}"),
        };
        let mut body = json!({"call_id": call, "status": "ok", "blocks": [{"type": "text", "text": text}], "duration_ms": 5});
        if !effects.is_empty() {
            body["effects"] = json!(effects);
        }
        let by = format!(r#"{{"kind":"tool","call_id":"{call}"}}"#);
        self.push("tool.result", Some(turn), &by, &body, shape);
    }

    /// 脚本里调的一个工具：挂在一次调用或者一个后台任务下面。
    fn nested(&mut self, dice: &mut Dice, shape: Shape) {
        let job = (!self.running.is_empty()).then(|| dice.pick(&self.running).0.clone());
        let parent = match (dice.odds(3), job, self.calls.last()) {
            (false, Some(job), _) => job,
            (_, _, Some(call)) => call.clone(),
            (_, Some(job), None) => job,
            (_, None, None) => return,
        };
        let n = 1 + dice.below(4);
        let status = *dice.pick(&["ok", "cancelled", "needs_approval", "error"]);
        let body = json!({"id": format!("{parent}#{n}"), "name": "kb_search", "args": "{\"query\":\"q\"}", "status": status, "duration_ms": 3});
        self.push("tool.nested", None, KERNEL, &body, shape);
    }

    /// 一个任务报了：后台命令、子代理；有时照它开下一轮。
    fn report(&mut self, dice: &mut Dice, shape: Shape) {
        if self.running.is_empty() {
            self.nested(dice, shape);
            return;
        }
        let k = usize::try_from(dice.below(self.running.len() as u64)).unwrap_or(0);
        let (job, agent, call) = self.running.remove(k);
        let seq = if agent {
            let reason = *dice.pick(&["done", "stopped", "undone", "aborted"]);
            let by = format!(r#"{{"kind":"session","id":"{PARENT}"}}"#);
            let body = json!({"job": job, "session": PARENT, "reason": reason, "text": "做完了"});
            self.push("child.reported", None, &by, &body, shape)
        } else {
            let reason = *dice.pick(&[
                "exited",
                "exited",
                "stopped",
                "undone",
                "restarted",
                "timed_out",
                "aborted",
            ]);
            let by = format!(r#"{{"kind":"tool","call_id":"{call}"}}"#);
            let mut body = json!({"job": job, "reason": reason, "duration_ms": 1200});
            if reason == "exited" {
                body["exit_code"] = json!(dice.below(2));
            }
            self.push("job.reported", None, &by, &body, shape)
        };
        self.reports.push(seq);
        if self.open.is_none() && dice.odds(2) {
            let started = self.next();
            self.push(
                "turn.started",
                Some(started),
                KERNEL,
                &json!({"trigger": seq}),
                Shape::Written,
            );
            self.open = Some(started);
            self.turns.push(started);
            self.undone.clear();
        }
    }

    /// 结束这一轮。
    fn end(&mut self, dice: &mut Dice, turn: u64, shape: Shape) {
        let reason = if dice.odds(4) {
            "interrupted"
        } else {
            "completed"
        };
        self.push(
            "turn.ended",
            Some(turn),
            KERNEL,
            &json!({"reason": reason}),
            shape,
        );
        self.open = None;
        self.queued.clear();
    }

    /// 撤回排着的话。
    fn withdraw(&mut self, turn: u64, shape: Shape) {
        if self.queued.is_empty() {
            return;
        }
        let messages = std::mem::take(&mut self.queued);
        self.push(
            "message.withdrawn",
            Some(turn),
            PERSON,
            &json!({"messages": messages}),
            shape,
        );
    }

    /// 撤最近一轮、最近几轮或者很早的一轮。
    fn revert(&mut self, dice: &mut Dice, shape: Shape) {
        if self.turns.is_empty() {
            return;
        }
        let take = match dice.below(3) {
            0 => 1,
            1 => 1 + dice.below(3),
            _ => self.turns.len() as u64,
        };
        let keep = self
            .turns
            .len()
            .saturating_sub(usize::try_from(take).unwrap_or(1));
        let gone: Vec<u64> = self.turns.split_off(keep);
        self.push(
            "turn.reverted",
            None,
            PERSON,
            &json!({"turns": gone}),
            shape,
        );
        self.undone.push(gone);
    }

    /// 恢复刚撤的。
    fn unrevert(&mut self, shape: Shape) {
        let Some(back) = self.undone.pop() else {
            return;
        };
        self.push(
            "turn.unreverted",
            None,
            PERSON,
            &json!({"turns": back}),
            shape,
        );
        self.turns.extend(back);
    }

    /// 撤了最近一轮接着重说一遍。
    fn redo(&mut self, dice: &mut Dice, shape: Shape) {
        let Some(&last) = self.turns.last() else {
            return;
        };
        self.turns.pop();
        self.push(
            "turn.reverted",
            None,
            PERSON,
            &json!({"turns": [last]}),
            shape,
        );
        self.say(dice, Shape::Written);
        self.start(dice, Shape::Written);
    }

    /// 压缩。
    fn compact(&mut self, shape: Shape) {
        let upto = self.next() - 1;
        self.push(
            "context.compacted",
            None,
            PERSON,
            &json!({"upto": upto, "summary": "前面说了这些。"}),
            shape,
        );
        self.undone.clear();
    }

    /// 改标题、斜杠命令、不认识的种类。
    fn misc(&mut self, dice: &mut Dice, shape: Shape) {
        match dice.below(3) {
            0 => self.push(
                "session.meta_changed",
                None,
                PERSON,
                &json!({"title": "标题"}),
                shape,
            ),
            1 => self.push(
                "command.ran",
                None,
                PERSON,
                &json!({"text": "/clear", "command": "clear"}),
                shape,
            ),
            _ => self.push(
                "ext.memory.recalled",
                self.open,
                KERNEL,
                &json!({"hits": [{"id": "m1"}]}),
                shape,
            ),
        };
    }
}

/// 一行写成什么样子。
#[derive(Clone, Copy)]
enum Shape {
    /// 内核写的样子。
    Written,
    /// 种类写在 `body` 后面。
    KindLast,
    /// `at` 写在 `seq` 前面。
    AtFirst,
    /// 效果的种类写成转义的。
    Escaped,
}

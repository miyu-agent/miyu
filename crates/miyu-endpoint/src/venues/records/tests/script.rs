//! 照种子造群会话的日志（施工 V-39 上的对照测试用）：群友、主人在群里说话（旁听的、叫她的、睡着时收到的、补拉回来的、带
//! 引用的——引用很早以前的、引用日志里没有的、带图的），开回合（照刚才一句、照很早以前一句旁听的、不带触发的、紧接着上一轮
//! 结束开的），回合里排着的话、她的回复和请求、她发进群里的话，撤回，撤销（最近一轮、最近几轮、很早的一轮）、恢复，撤回排着
//! 的话，压缩；偶尔撤一轮日志里没有的、触发写成还没有的序号（骨架认不得）。一行一条，序号从 1 数、接着往下数。
//!
//! 施工 V-39 上补加了判官不要的种类：工具结果、注入的事实、不认识的种类、暂停着没发出去的请求（撤销查排着的话时不算），
//! 回合里到的回报、子会话回报、等的会话空了（照它们接着开下一轮的，撤销时认紧挨着结束的上一轮）；每 11 条有一条种类写在
//! `body` 后面。

/// 照种子走的随机数（xorshift，同 V-37 下的对照测试）。
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
}

const MEMBERS: [&str; 3] = [
    r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#,
    r#"{"kind":"external","venue":"qq:group:1","id":"qq:20018","role":"manager"}"#,
    r#"{"kind":"person","via":"qq:10001"}"#,
];
const KERNEL: &str = r#"{"kind":"kernel"}"#;
const BRIDGE: &str = r#"{"kind":"module","id":"onebot"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const TOOL: &str = r#"{"kind":"tool","call_id":"call_5_1"}"#;
const PEER: &str = r#"{"kind":"session","id":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c"}"#;
const CREATED: &str = r#"{"venue":"qq:group:1","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

/// 造着的一段日志。
pub(super) struct Script {
    /// 写好的行。
    pub(super) lines: Vec<String>,
    /// 群里的话：序号。
    said: Vec<u64>,
    /// 平台编号用到第几个。
    ids: u64,
    /// 正在跑的那一轮。
    open: Option<u64>,
    /// 开过的、没撤掉的回合，照先后。
    turns: Vec<u64>,
    /// 刚撤的那几轮（还没开新的一轮、没压缩），能恢复。
    undone: Vec<Vec<u64>>,
    /// 上一条人话的命令编号。
    cause: u64,
    /// 回报、子会话回报、等的会话空了：序号。
    reports: Vec<u64>,
}

impl Script {
    /// 第一条是 `session.created`。
    pub(super) fn new() -> Script {
        let mut script = Script {
            lines: Vec::new(),
            said: Vec::new(),
            ids: 0,
            open: None,
            turns: Vec::new(),
            undone: Vec::new(),
            cause: 0,
            reports: Vec::new(),
        };
        script.line("session.created", None, MEMBERS[2], None, CREATED);
        script
    }

    /// 下一条的序号。
    fn next(&self) -> u64 {
        self.lines.len() as u64 + 1
    }

    fn line(&mut self, kind: &str, turn: Option<u64>, by: &str, cause: Option<u64>, body: &str) {
        let seq = self.next();
        let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
        let cause = cause.map_or(String::new(), |cause| format!(r#","cause":"c{cause}""#));
        let at = format!(
            "2026-11-03T0{}:{:02}:{:02}.000Z",
            seq / 3600 % 10,
            seq / 60 % 60,
            seq % 60
        );
        // 每 11 条有一条种类写在 `body` 后面（不是核心写的样子，照样读得懂）。
        self.lines.push(if seq.is_multiple_of(11) {
            format!(
                r#"{{"seq":{seq},"at":"{at}"{turn},"by":{by}{cause},"body":{body},"kind":"{kind}"}}"#
            )
        } else {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"{kind}"{turn},"by":{by}{cause},"body":{body}}}"#
            )
        });
    }

    /// 照种子写一段：`steps` 步。
    pub(super) fn run(dice: &mut Dice, steps: u64) -> Script {
        let mut script = Script::new();
        for _ in 0..steps {
            script.step(dice);
        }
        script
    }

    fn step(&mut self, dice: &mut Dice) {
        match dice.below(20) {
            0..=7 => self.say(dice),
            8..=9 if self.open.is_none() => self.start(dice),
            8..=11 if self.open.is_some() => self.inside(dice),
            12..=13 if self.open.is_some() => self.end(dice),
            14 => {
                let msg = self.old_id(dice);
                let by = 17 + dice.below(2);
                self.line(
                    "venue.recalled",
                    None,
                    BRIDGE,
                    None,
                    &format!(r#"{{"msg":"{msg}","by":"qq:200{by}"}}"#),
                );
            }
            15 if self.open.is_none() => self.revert(dice),
            16 if !self.undone.is_empty() => {
                let turns = self.undone.pop().expect("有撤的");
                self.turns.extend(&turns);
                self.turns.sort_unstable();
                self.line(
                    "turn.unreverted",
                    None,
                    MEMBERS[2],
                    None,
                    &format!(r#"{{"turns":{turns:?}}}"#),
                );
            }
            17 => self.withdraw(dice),
            18 if dice.odds(4) => {
                let upto = 1 + dice.below(self.next() - 1);
                self.undone.clear();
                self.line(
                    "context.compacted",
                    self.open,
                    KERNEL,
                    None,
                    &format!(r#"{{"upto":{upto},"summary":"s"}}"#),
                );
            }
            _ => self.say(dice),
        }
    }

    /// 一个平台编号：多半是新的，偶尔撞上以前的。
    fn id(&mut self, dice: &mut Dice) -> String {
        if self.ids > 0 && dice.odds(12) {
            return format!("m{}", dice.below(self.ids));
        }
        self.ids += 1;
        format!("m{}", self.ids)
    }

    /// 以前用过的一个编号（很早的也有），偶尔是没有的。
    fn old_id(&mut self, dice: &mut Dice) -> String {
        if self.ids == 0 || dice.odds(8) {
            return "m-none".to_string();
        }
        match dice.below(3) {
            0 => format!("m{}", 1 + dice.below(self.ids.min(4))),
            _ => format!("m{}", self.ids - dice.below(self.ids.min(6))),
        }
    }

    fn say(&mut self, dice: &mut Dice) {
        let by = dice.below(3) as usize;
        let mut venue = format!(r#""msg":"{}","name":"群友{by}""#, self.id(dice));
        for (flag, odds) in [
            ("ambient", 2),
            ("asleep", 9),
            ("backfill", 11),
            ("show_ids", 3),
            ("mentions_me", 4),
        ] {
            if dice.odds(odds) {
                venue.push_str(&format!(r#","{flag}":true"#));
            }
        }
        if dice.odds(3) {
            let reply_to = self.old_id(dice);
            venue.push_str(&format!(r#","reply_to":"{reply_to}""#));
        }
        if dice.odds(6) {
            venue.push_str(r#","media":[{"kind":"image","id":"i1"}]"#);
        }
        let words = ["早", "", "@Miyu 在吗", "a\\nb", "饭有点咸"][dice.below(5) as usize];
        let body =
            format!(r#"{{"blocks":[{{"type":"text","text":"{words}"}}],"venue":{{{venue}}}}}"#);
        // 排着的话带这一轮的编号；闲着时说的不带，偶尔和上一句同一个命令（重做重发的几句）。
        let turn = self.open.filter(|_| !dice.odds(4));
        if !(turn.is_none() && dice.odds(4)) {
            self.cause += 1;
        }
        let seq = self.next();
        self.line("message.user", turn, MEMBERS[by], Some(self.cause), &body);
        self.said.push(seq);
    }

    fn start(&mut self, dice: &mut Dice) {
        let seq = self.next();
        let report = self.reports.last().copied().filter(|_| dice.odds(3));
        let body = match (dice.below(8), self.said.last(), report) {
            // 照回合里到的回报、子会话回报、等的会话空了开的。
            (_, _, Some(report)) => format!(r#"{{"trigger":{report}}}"#),
            (0, _, _) | (_, None, _) => "{}".to_string(),
            (1..=2, Some(_), _) => {
                // 照很早以前的一句旁听的开（`session.respond`）。
                let old = self.said[dice.below(self.said.len() as u64) as usize];
                format!(r#"{{"trigger":{old},"triggers":[{old}]}}"#)
            }
            (_, Some(&last), _) => format!(r#"{{"trigger":{last}}}"#),
        };
        let body = if dice.odds(150) {
            format!(r#"{{"trigger":{}}}"#, seq + 5)
        } else {
            body
        };
        self.undone.clear();
        self.line("turn.started", Some(seq), KERNEL, None, &body);
        self.open = Some(seq);
        self.turns.push(seq);
    }

    fn inside(&mut self, dice: &mut Dice) {
        let turn = self.open.expect("在一轮里");
        let seen = self.next() - 1 - dice.below(3).min(self.next() - 2);
        match dice.below(6) {
            0 => self.line(
                "message.assistant",
                Some(turn),
                MODEL,
                None,
                &format!(r#"{{"blocks":[{{"type":"text","text":"好"}}],"seen":{seen}}}"#),
            ),
            1 => self.line(
                "model.called",
                Some(turn),
                KERNEL,
                None,
                &format!(r#"{{"seen":{seen},"messages":1,"result":"ok"}}"#),
            ),
            3 => self.unwanted(dice, turn, seen),
            4 => self.report(dice),
            _ => {
                let msg = self.id(dice);
                self.line(
                    "venue.delivered",
                    Some(turn),
                    BRIDGE,
                    None,
                    &format!(
                        r#"{{"line":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","turn":{turn},"to":[],"msg":"{msg}","text":"在"}}"#
                    ),
                );
            }
        }
    }

    /// 判官不要的种类（施工 V-39 上补）：工具结果、注入的事实、不认识的种类、暂停着没发出去的请求（`seen` 照样写，撤销查
    /// 排着的话时不算它）。
    fn unwanted(&mut self, dice: &mut Dice, turn: u64, seen: u64) {
        let call = self.next() - 1;
        match dice.below(4) {
            0 => self.line(
                "tool.result",
                Some(turn),
                TOOL,
                None,
                &format!(
                    r#"{{"call_id":"call_{call}_1","status":"ok","blocks":[{{"type":"text","text":"lib.rs\nmain.rs"}}],"duration_ms":12}}"#
                ),
            ),
            1 => self.line(
                "context.injected",
                Some(turn),
                KERNEL,
                None,
                r#"{"kind":"env","text":"<env cwd=\"~\"/>\n"}"#,
            ),
            2 => self.line("ext.note", Some(turn), BRIDGE, None, r#"{"x":[1,{"y":"z"}]}"#),
            _ => self.line(
                "model.called",
                Some(turn),
                KERNEL,
                None,
                &format!(
                    r#"{{"seen":{seen},"messages":1,"result":"error","error":{{"class":"compaction_paused","message":"放不下"}}}}"#
                ),
            ),
        }
    }

    /// 回合里到的回报、子会话回报、等的会话空了（不带回合编号）：下一轮可能照它接着开。
    fn report(&mut self, dice: &mut Dice) {
        let seq = self.next();
        match dice.below(3) {
            0 => self.line(
                "job.reported",
                None,
                TOOL,
                Some(9000 + seq),
                r#"{"job":"j1","reason":"exited","exit_code":0,"duration_ms":5,"output":"sha256:3245795a51c1f9e64adb119c6b258a9079a4fdc7f1a15677f1d15d09910bef9c","chars":4}"#,
            ),
            1 => self.line(
                "child.reported",
                None,
                PEER,
                Some(9000 + seq),
                r#"{"job":"j2","session":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c","reason":"done","text":"好了"}"#,
            ),
            _ => self.line(
                "peer.idle",
                None,
                KERNEL,
                Some(9000 + seq),
                r#"{"session":"01a09b7e-3c15-7a62-b4e1-2f8e0c5d77aa","reason":"expired"}"#,
            ),
        }
        self.reports.push(seq);
    }

    fn end(&mut self, dice: &mut Dice) {
        let turn = self.open.take().expect("在一轮里");
        self.line(
            "turn.ended",
            Some(turn),
            KERNEL,
            None,
            r#"{"reason":"completed"}"#,
        );
        // 排着的话接着开下一轮：两条挨着。
        if dice.odds(3) {
            self.start(dice);
        }
    }

    fn revert(&mut self, dice: &mut Dice) {
        let turns: Vec<u64> = if dice.odds(200) {
            vec![self.next() + 7]
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
        self.line(
            "turn.reverted",
            None,
            MEMBERS[2],
            None,
            &format!(r#"{{"turns":{turns:?}}}"#),
        );
        self.undone.push(turns);
    }

    fn withdraw(&mut self, dice: &mut Dice) {
        let picked: Vec<u64> = self
            .said
            .iter()
            .rev()
            .take(4)
            .copied()
            .filter(|_| dice.odds(2))
            .collect();
        let mut picked = picked;
        picked.sort_unstable();
        if dice.odds(10) && !self.said.is_empty() {
            picked.insert(0, self.said[dice.below(self.said.len() as u64) as usize]);
            picked.sort_unstable();
            picked.dedup();
        }
        self.line(
            "message.withdrawn",
            self.open,
            KERNEL,
            None,
            &format!(r#"{{"messages":{picked:?}}}"#),
        );
    }
}

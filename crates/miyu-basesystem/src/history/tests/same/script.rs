//! 照种子造会话日志（施工 V-45 的对照测试用，照问话列表的生成器 `miyu-endpoint` 的 `asked/tests/script.rs` 扩）：主人闲着
//! 说的（自己开一轮）、回合里排着的、桥代主人转进来的（带 `via`、带 `venue`，补拉回来的带平台的时刻）；群友（名字、平台身份、
//! `show_ids`、管理员、主人）、别的 harness（名字带引号）、别的会话、父会话、派出去的子代理、模块、定时、内核说的；她的回复
//! （正文、思考、图、文件、工具调用：`history` 和别的工具，一条里几个）、请求、工具结果（翻记录的、别的，正文空的、只有空白的、
//! 比一页还长的，派子代理的带任务效果）、注入的事实、不认识的种类；人切权限级别（带、不带 `permission`）；回报接着开的；撤销
//! （最近一轮、几轮、很早的一轮）、恢复、重做、撤回排着的话、压缩（摘要空的、撤销跨压缩）。正文里大小写混着、中文、转成小写会变
//! 长的字（`İ`、开尔文符号）、照上下文转的希腊字母 Σ、连着的空白。偶尔撤一轮日志里没有的、触发写成还没有的序号（骨架认不得）；偶尔回复里调用编号不是
//! 自己的序号、工具结果的编号在它后面（账本过不了，`collect.rs` 退回整份读）。子会话、分叉出来的从 `upto + 1` 数起。

use serde_json::{Value, json};

/// 照种子走的随机数（xorshift，同判官、问话列表的对照测试）。
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
    pub(super) fn pick<'a, T>(&mut self, from: &'a [T]) -> &'a T {
        &from[self.below(from.len() as u64) as usize]
    }
}

/// 正文里的词：找的时候也照它们挑。
pub(super) const WORDS: [&str; 18] = [
    "omega",
    "Omega",
    "OMEGA",
    "beta",
    "分区",
    "数据库",
    "İstanbul",
    "\u{212A}elvin",
    "kelvin",
    "→",
    "shell",
    "表",
    "[image]",
    "a.rs",
    "x\"y",
    "ǅemal",
    "ΣΑΣ",
    "σας",
];

const PERSON: &str = r#"{"kind":"person"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const POLICY: &str = r#""policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"full","read_only":false}"#;
const BLOB: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
/// 别的会话、父会话、子代理的会话编号：短编号各不同。
pub(super) const PEER: &str = "0192f3a0-1111-7abc-8def-001122334455";
const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
const AGENTS: [&str; 2] = [
    "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91",
    "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d92",
];

/// 开过的一轮：编号，开它的人话（序号、命令编号）。
type Turn = (u64, Option<(u64, u64)>);

/// 造着的一段日志。
pub(super) struct Script {
    /// 写好的行。
    pub(super) lines: Vec<String>,
    /// 第一条的序号。
    pub(super) first: u64,
    said: Vec<(u64, u64)>,
    open: Option<u64>,
    turns: Vec<Turn>,
    undone: Vec<Vec<Turn>>,
    cause: u64,
    reports: Vec<u64>,
    /// 等着结果的调用编号。
    pending: Vec<String>,
}

/// 第 `seq` 条写下的时刻：一条隔 37 秒，跨天。
pub(super) fn at(seq: u64) -> String {
    let secs = seq * 37;
    format!(
        "2026-11-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        3 + secs / 86_400,
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60,
        seq % 1000
    )
}

impl Script {
    /// 照种子写一段：`steps` 步。`child` 的是子会话（`session.created` 带父会话），`forked` 的从第 `upto + 1` 条数起。
    pub(super) fn run(dice: &mut Dice, steps: u64, child: bool, forked: Option<u64>) -> Script {
        let first = forked.map_or(1, |upto| upto + 1);
        let mut script = Script {
            lines: Vec::new(),
            first,
            said: Vec::new(),
            open: None,
            turns: Vec::new(),
            undone: Vec::new(),
            cause: 0,
            reports: Vec::new(),
            pending: Vec::new(),
        };
        let parent = if child || forked.is_some() {
            format!(r#","parent":"{PARENT}""#)
        } else {
            String::new()
        };
        let fork = forked.map_or(String::new(), |upto| {
            format!(r#","forked":{{"from":"{PARENT}","upto":{upto}}}"#)
        });
        script.line(
            "session.created",
            None,
            PERSON,
            None,
            &format!(r#"{{"venue":"local",{POLICY}{parent}{fork}}}"#),
        );
        for _ in 0..steps {
            script.step(dice);
        }
        script
    }

    fn next(&self) -> u64 {
        self.first + self.lines.len() as u64
    }

    fn line(&mut self, kind: &str, turn: Option<u64>, by: &str, cause: Option<u64>, body: &str) {
        let seq = self.next();
        let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
        let cause = cause.map_or(String::new(), |cause| format!(r#","cause":"c{cause}""#));
        let at = at(seq);
        self.lines.push(format!(
            r#"{{"seq":{seq},"at":"{at}","kind":"{kind}"{turn},"by":{by}{cause},"body":{body}}}"#
        ));
    }

    fn step(&mut self, dice: &mut Dice) {
        match dice.below(26) {
            0..=3 => self.say(dice),
            4..=5 => self.other(dice),
            6..=7 if self.open.is_none() => self.start(dice, None),
            6..=12 if self.open.is_some() => self.inside(dice),
            13..=14 if self.open.is_some() => self.end(dice),
            15 if self.open.is_none() => self.revert(dice),
            16 if !self.undone.is_empty() => {
                let turns = self.undone.pop().unwrap_or_default();
                let numbers: Vec<u64> = turns.iter().map(|turn| turn.0).collect();
                self.turns.extend(turns);
                self.turns.sort_unstable();
                let body = format!(r#"{{"turns":{numbers:?}}}"#);
                self.line("turn.unreverted", None, PERSON, None, &body);
            }
            17 if self.open.is_none() => self.redo(dice),
            18 => self.withdraw(dice),
            19 if dice.odds(2) => {
                let upto = self.first + dice.below(self.next() - self.first);
                self.undone.clear();
                let summary = Script::text(dice);
                let body = json!({"upto": upto, "summary": summary}).to_string();
                self.line("context.compacted", self.open, KERNEL, None, &body);
            }
            20 => self.report(dice),
            21 => {
                let body = match dice.below(4) {
                    0 => format!(r#"{{"policy":"{BLOB}"}}"#),
                    1 => r#"{"permission":{"level":"read_only","read_only":true}}"#.to_string(),
                    2 => r#"{"permission":{"level":"full","read_only":false}}"#.to_string(),
                    _ => r#"{"permission":{"level":"full","read_only":true}}"#.to_string(),
                };
                self.line("session.policy_changed", None, PERSON, None, &body);
            }
            _ => self.say(dice),
        }
    }

    /// 一段正文：几个词、中间的空白各式各样；偶尔空的、只有空白的、比一页还长的。
    pub(super) fn text(dice: &mut Dice) -> String {
        match dice.below(200) {
            0..=9 => String::new(),
            10..=19 => " \n\t ".to_string(),
            20 => format!("{} omega {}", "长".repeat(15_000), "Beta ".repeat(3_500)),
            21 => "x".repeat(31_000),
            _ => {
                let mut text = String::new();
                for k in 0..1 + dice.below(12) {
                    if k > 0 {
                        text.push_str(dice.pick(&[" ", "  ", "\n", " \n\n\t", "，"]));
                    }
                    text.push_str(dice.pick(&WORDS));
                }
                if dice.odds(4) {
                    text.push_str(&" filler".repeat(dice.below(60) as usize));
                }
                text
            }
        }
    }

    fn blocks(dice: &mut Dice) -> Vec<Value> {
        let mut blocks = Vec::new();
        for _ in 0..1 + dice.below(3) {
            blocks.push(match dice.below(8) {
                0 => json!({"type": "image", "blob": BLOB, "media_type": "image/png", "width": 1, "height": 1}),
                1 => json!({"type": "file", "blob": BLOB, "name": "plan \"v2\".pdf", "media_type": "application/pdf"}),
                _ => json!({"type": "text", "text": Script::text(dice)}),
            });
        }
        blocks
    }

    /// 主人说一句：在一轮里的排着（带这一轮的编号），闲着的不带、自己开一轮。偶尔经桥转进来、带平台给的时刻。
    fn say(&mut self, dice: &mut Dice) {
        self.cause += 1;
        let cause = self.cause;
        let seq = self.next();
        let mut body = json!({"blocks": Script::blocks(dice)});
        let by = if dice.odds(5) {
            body["venue"] = Script::venue(dice, seq);
            r#"{"kind":"person","via":"qq:10001"}"#
        } else {
            PERSON
        };
        let turn = self.open;
        self.line("message.user", turn, by, Some(cause), &body.to_string());
        self.said.push((seq, cause));
        if turn.is_none() && !dice.odds(5) {
            self.start(dice, Some((seq, cause)));
        }
    }

    /// 场所消息带的：平台编号、名字（偶尔没有、带引号）、`show_ids`，偶尔补拉回来、平台的时刻早几分钟。
    fn venue(dice: &mut Dice, seq: u64) -> Value {
        let mut venue = json!({"msg": format!("m{seq}")});
        if !dice.odds(3) {
            venue["name"] = json!(dice.pick(&["小林", "老\"王\"", "Ann\nB"]));
        }
        if dice.odds(2) {
            venue["show_ids"] = json!(true);
        }
        if dice.odds(4) {
            venue["backfill"] = json!(true);
            venue["sent"] = json!(at(seq.saturating_sub(1 + dice.below(40))));
        }
        venue
    }

    /// 别的来路的一句。
    fn other(&mut self, dice: &mut Dice) {
        self.cause += 1;
        let seq = self.next();
        let mut body = json!({"blocks": Script::blocks(dice)});
        let peer = format!(r#"{{"kind":"session","id":"{PEER}"}}"#);
        let parent = format!(r#"{{"kind":"session","id":"{PARENT}"}}"#);
        let agent = format!(r#"{{"kind":"session","id":"{}"}}"#, dice.pick(&AGENTS));
        let by = match dice.below(9) {
            0 => peer,
            1 => parent,
            2 => agent,
            3 => r#"{"kind":"harness","name":"claude \"code\""}"#.to_string(),
            4 => r#"{"kind":"module","id":"goal"}"#.to_string(),
            5 => r#"{"kind":"schedule","schedule":"t1"}"#.to_string(),
            _ => {
                body["venue"] = Script::venue(dice, seq);
                let role = dice.pick(&["member", "manager"]);
                let owner = if dice.odds(3) { r#","owner":true"# } else { "" };
                format!(
                    r#"{{"kind":"external","venue":"qq:group:1","id":"qq:2001{}","role":"{role}"{owner}}}"#,
                    dice.below(3)
                )
            }
        };
        let turn = self.open.filter(|_| dice.odds(2));
        self.line(
            "message.user",
            turn,
            &by,
            Some(self.cause),
            &body.to_string(),
        );
    }

    /// 开一轮：照主人刚说的那句、回报、别的、不带触发的；偶尔触发写成还没有的序号。
    fn start(&mut self, dice: &mut Dice, by: Option<(u64, u64)>) {
        let seq = self.next();
        let report = self.reports.last().copied().filter(|_| dice.odds(4));
        let (body, trigger) = match (by, report) {
            (Some((said, cause)), _) => (format!(r#"{{"trigger":{said}}}"#), Some((said, cause))),
            (None, Some(report)) => (format!(r#"{{"trigger":{report}}}"#), None),
            (None, None) => match self.said.last() {
                Some(&(said, cause)) if dice.odds(2) => {
                    (format!(r#"{{"trigger":{said}}}"#), Some((said, cause)))
                }
                _ => ("{}".to_string(), None),
            },
        };
        let body = if dice.odds(400) {
            format!(r#"{{"trigger":{}}}"#, seq + 5)
        } else {
            body
        };
        self.undone.clear();
        self.line("turn.started", Some(seq), KERNEL, None, &body);
        self.open = Some(seq);
        self.turns.push((seq, trigger));
    }

    fn inside(&mut self, dice: &mut Dice) {
        let turn = self.open.unwrap_or_default();
        let seen = self.next() - 1 - dice.below(3).min(self.next() - 1 - self.first);
        match dice.below(9) {
            0..=2 => self.reply(dice, turn, seen),
            3..=4 if !self.pending.is_empty() => self.result(dice, turn),
            5 => self.line(
                "model.called",
                Some(turn),
                KERNEL,
                None,
                &format!(r#"{{"seen":{seen},"messages":1,"result":"ok"}}"#),
            ),
            6 => self.line(
                "context.injected",
                Some(turn),
                KERNEL,
                None,
                r#"{"kind":"env","text":"<env/>"}"#,
            ),
            7 => self.line("ext.note", Some(turn), KERNEL, None, r#"{"x":[1]}"#),
            _ => self.say(dice),
        }
    }

    /// 她的一条回复：正文、思考、图，几个工具调用（`history`、别的）。偶尔调用编号不是自己的序号（账本过不了）。
    fn reply(&mut self, dice: &mut Dice, turn: u64, seen: u64) {
        let seq = self.next();
        let mut blocks = Vec::new();
        if dice.odds(3) {
            blocks.push(json!({"type": "reasoning", "text": "想想 omega"}));
        }
        if !dice.odds(4) {
            blocks.extend(Script::blocks(dice));
        }
        for k in 1..=dice.below(3) {
            let owner = if dice.odds(150) { seq + 2 } else { seq };
            let id = format!("call_{owner}_{k}");
            let (name, args) = match dice.below(3) {
                0 => ("history", json!({"query": dice.pick(&WORDS)}).to_string()),
                1 => (
                    "shell",
                    r#"{"command":"cat a.rs","description":"看 omega"}"#.to_string(),
                ),
                _ => ("read", r#"{"file_path":"表.txt"}"#.to_string()),
            };
            blocks.push(json!({"type": "tool_call", "call_id": id, "name": name, "args": args}));
            self.pending.push(id);
        }
        let mut body = json!({"blocks": blocks, "seen": seen});
        if dice.odds(8) {
            body["interrupted"] = json!(true);
        }
        self.line(
            "message.assistant",
            Some(turn),
            MODEL,
            None,
            &body.to_string(),
        );
    }

    /// 一个等着的调用的结果；偶尔派了子代理（带任务效果）。偶尔编号指到后面的回复（账本过不了）。
    fn result(&mut self, dice: &mut Dice, turn: u64) {
        let k = dice.below(self.pending.len() as u64) as usize;
        let mut id = self.pending.remove(k);
        if dice.odds(150) {
            id = format!("call_{}_1", self.next() + 3);
        }
        let mut body = json!({"call_id": id, "status": "ok", "blocks": Script::blocks(dice)});
        if dice.odds(6) {
            let session = dice.pick(&AGENTS);
            body["effects"] = json!([{"kind": "job.started", "job": format!("j{}", self.next()), "what": "agent", "title": "查", "session": session}]);
        }
        let by = format!(r#"{{"kind":"tool","call_id":"{id}"}}"#);
        self.line("tool.result", Some(turn), &by, None, &body.to_string());
    }

    /// 回报、子会话回报（不带回合编号）：下一轮可能照它接着开。
    fn report(&mut self, dice: &mut Dice) {
        let seq = self.next();
        match dice.below(2) {
            0 => self.line(
                "job.reported",
                None,
                r#"{"kind":"tool","call_id":"call_5_1"}"#,
                Some(9000 + seq),
                r#"{"job":"j1","reason":"exited","exit_code":0,"duration_ms":5,"output":"sha256:3245795a51c1f9e64adb119c6b258a9079a4fdc7f1a15677f1d15d09910bef9c","chars":4}"#,
            ),
            _ => self.line(
                "child.reported",
                None,
                &format!(r#"{{"kind":"session","id":"{}"}}"#, AGENTS[0]),
                Some(9000 + seq),
                &format!(r#"{{"job":"j2","session":"{}","reason":"done","text":"好了"}}"#, AGENTS[0]),
            ),
        }
        self.reports.push(seq);
    }

    fn end(&mut self, dice: &mut Dice) {
        let turn = self.open.take().unwrap_or_default();
        self.pending.clear();
        self.line(
            "turn.ended",
            Some(turn),
            KERNEL,
            None,
            r#"{"reason":"completed"}"#,
        );
        if dice.odds(3) {
            self.start(dice, None);
        }
    }

    /// 撤几轮。偶尔撤一轮日志里没有的。
    fn revert(&mut self, dice: &mut Dice) {
        let turns = if dice.odds(400) {
            vec![(self.next() + 7, None)]
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
        let numbers: Vec<u64> = turns.iter().map(|turn| turn.0).collect();
        let body = format!(r#"{{"turns":{numbers:?}}}"#);
        self.line("turn.reverted", None, PERSON, None, &body);
        self.undone.push(turns);
    }

    /// 重做（编辑重发）：撤掉最后一轮，开它的人话同一个命令重发，再开一轮。
    fn redo(&mut self, dice: &mut Dice) {
        let Some(&(turn, Some((_, cause)))) = self.turns.last() else {
            return;
        };
        self.turns.pop();
        let body = format!(r#"{{"turns":[{turn}]}}"#);
        self.line("turn.reverted", None, PERSON, None, &body);
        let seq = self.next();
        let body = json!({"blocks": Script::blocks(dice)}).to_string();
        self.line("message.user", None, PERSON, Some(cause), &body);
        self.said.push((seq, cause));
        self.start(dice, Some((seq, cause)));
    }

    /// 撤回排着的话：最近几句里挑，偶尔挑很早的。
    fn withdraw(&mut self, dice: &mut Dice) {
        let mut picked: Vec<u64> = self
            .said
            .iter()
            .rev()
            .take(4)
            .map(|said| said.0)
            .filter(|_| dice.odds(2))
            .collect();
        if dice.odds(10) && !self.said.is_empty() {
            picked.push(self.said[dice.below(self.said.len() as u64) as usize].0);
        }
        picked.sort_unstable();
        picked.dedup();
        let body = format!(r#"{{"messages":{picked:?}}}"#);
        self.line("message.withdrawn", self.open, KERNEL, None, &body);
    }
}

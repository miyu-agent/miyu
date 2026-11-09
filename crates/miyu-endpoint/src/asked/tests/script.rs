//! 照种子造本机会话的日志（施工 SP-2 的对照测试用）：主人闲着时说的（自己开一轮）、回合里排着的、斜杠命令模板展开的、多行的、
//! 很长的、只有图的，偶尔是桥代主人在私聊里转进来的（带 `via`）；别的来路的话（平台上别的人、子会话和父会话发来的、模块注入
//! 的、别的 harness、定时的）；回合
//! 里她的回复和请求（有暂停着没发出去的）、工具结果、注入的事实、不认识的种类；回报、子会话回报、等的会话空了（照它们接着开
//! 下一轮）；撤销（最近一轮、最近几轮、很早的一轮）、恢复、重做（撤掉最后一轮，同一个命令重发再开一轮）、撤回排着的话、
//! 压缩（撤销跨压缩）；偶尔撤一轮日志里没有的、触发写成还没有的序号（骨架认不得）。分叉出来的从 `upto + 1` 数起。

/// 照种子走的随机数（xorshift，同判官的对照测试）。
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

const PERSON: &str = r#"{"kind":"person"}"#;
const VIA: &str = r#"{"kind":"person","via":"qq:10001"}"#;
const OTHERS: [&str; 6] = [
    r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#,
    r#"{"kind":"session","id":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c"}"#,
    r#"{"kind":"module","id":"goal"}"#,
    r#"{"kind":"harness","name":"claude"}"#,
    r#"{"kind":"schedule","schedule":"t1"}"#,
    r#"{"kind":"kernel"}"#,
];
const KERNEL: &str = r#"{"kind":"kernel"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const TOOL: &str = r#"{"kind":"tool","call_id":"call_5_1"}"#;
const PEER: &str = r#"{"kind":"session","id":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c"}"#;
const POLICY: &str = r#""policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"full","read_only":false}"#;

/// 开过的一轮：编号，开它的人话（序号、命令编号）。
type Turn = (u64, Option<(u64, u64)>);

/// 造着的一段日志。
pub(super) struct Script {
    /// 写好的行。
    pub(super) lines: Vec<String>,
    /// 第一条的序号。
    first: u64,
    /// 主人的话：序号、命令编号。
    said: Vec<(u64, u64)>,
    /// 正在跑的那一轮。
    open: Option<u64>,
    /// 开过的、没撤掉的回合，照先后，各带开它的人话（命令编号）。
    turns: Vec<Turn>,
    /// 刚撤的那几轮（还没开新的一轮、没压缩），能恢复。
    undone: Vec<Vec<Turn>>,
    /// 命令编号用到第几个。
    cause: u64,
    /// 回报、子会话回报、等的会话空了：序号。
    reports: Vec<u64>,
}

impl Script {
    /// 第一条是 `session.created`；`forked` 的从第 `upto + 1` 条数起。
    pub(super) fn new(forked: Option<u64>) -> Script {
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
        };
        let created = match forked {
            Some(upto) => format!(
                r#"{{"venue":"local",{POLICY},"parent":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c","forked":{{"from":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c","upto":{upto}}}}}"#
            ),
            None => format!(r#"{{"venue":"local",{POLICY}}}"#),
        };
        script.line("session.created", None, PERSON, None, &created);
        script
    }

    /// 下一条的序号。
    fn next(&self) -> u64 {
        self.first + self.lines.len() as u64
    }

    fn line(&mut self, kind: &str, turn: Option<u64>, by: &str, cause: Option<u64>, body: &str) {
        let seq = self.next();
        let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
        let cause = cause.map_or(String::new(), |cause| format!(r#","cause":"c{cause}""#));
        let at = format!(
            "2026-11-03T0{}:{:02}:{:02}.{:03}Z",
            seq / 3600 % 10,
            seq / 60 % 60,
            seq % 60,
            seq % 1000
        );
        self.lines.push(format!(
            r#"{{"seq":{seq},"at":"{at}","kind":"{kind}"{turn},"by":{by}{cause},"body":{body}}}"#
        ));
    }

    /// 照种子写一段：`steps` 步。
    pub(super) fn run(dice: &mut Dice, steps: u64, forked: Option<u64>) -> Script {
        let mut script = Script::new(forked);
        for _ in 0..steps {
            script.step(dice);
        }
        script
    }

    fn step(&mut self, dice: &mut Dice) {
        match dice.below(22) {
            0..=4 => self.say(dice),
            5..=6 => self.other(dice),
            7..=8 if self.open.is_none() => self.start(dice, None),
            7..=11 if self.open.is_some() => self.inside(dice),
            12..=13 if self.open.is_some() => self.end(dice),
            14 if self.open.is_none() => {
                self.revert(dice);
            }
            15 if !self.undone.is_empty() => {
                let turns = self.undone.pop().expect("有撤的");
                let numbers: Vec<u64> = turns.iter().map(|turn| turn.0).collect();
                self.turns.extend(turns);
                self.turns.sort_unstable();
                self.line(
                    "turn.unreverted",
                    None,
                    PERSON,
                    None,
                    &format!(r#"{{"turns":{numbers:?}}}"#),
                );
            }
            16 if self.open.is_none() => self.redo(dice),
            17 => self.withdraw(dice),
            18 if dice.odds(3) => {
                let upto = self.first + dice.below(self.next() - self.first);
                self.undone.clear();
                self.line(
                    "context.compacted",
                    self.open,
                    KERNEL,
                    None,
                    &format!(r#"{{"upto":{upto},"summary":"s"}}"#),
                );
            }
            19 => self.report(dice),
            _ => self.say(dice),
        }
    }

    /// 主人的一句的正文。
    fn words(dice: &mut Dice) -> String {
        match dice.below(7) {
            0 => r#"{"blocks":[{"type":"text","text":"  \n 第一行 \n第二行"}]}"#.to_string(),
            1 => format!(
                r#"{{"blocks":[{{"type":"text","text":"{}"}}]}}"#,
                "长".repeat(80 + dice.below(60) as usize)
            ),
            2 => r#"{"blocks":[{"type":"text","text":"Review a.rs carefully."}],"command":{"name":"review","text":"/review a.rs"}}"#.to_string(),
            3 => r#"{"blocks":[{"type":"image","blob":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","media_type":"image/png","width":1,"height":1}]}"#.to_string(),
            _ => format!(
                r#"{{"blocks":[{{"type":"text","text":"问 {}"}}]}}"#,
                dice.below(1000)
            ),
        }
    }

    /// 主人说一句：在一轮里的排着（带这一轮的编号），闲着的不带、自己开一轮。
    fn say(&mut self, dice: &mut Dice) {
        self.cause += 1;
        let cause = self.cause;
        let seq = self.next();
        let body = Script::words(dice);
        let turn = self.open;
        let by = if dice.odds(6) { VIA } else { PERSON };
        self.line("message.user", turn, by, Some(cause), &body);
        self.said.push((seq, cause));
        if turn.is_none() && !dice.odds(5) {
            self.start(dice, Some((seq, cause)));
        }
    }

    /// 别的来路的一句。
    fn other(&mut self, dice: &mut Dice) {
        let by = OTHERS[dice.below(OTHERS.len() as u64) as usize];
        self.cause += 1;
        let turn = self.open.filter(|_| dice.odds(2));
        self.line(
            "message.user",
            turn,
            by,
            Some(self.cause),
            r#"{"blocks":[{"type":"text","text":"别处来的"}]}"#,
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
        let body = if dice.odds(300) {
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
        let turn = self.open.expect("在一轮里");
        let seen = self.next() - 1 - dice.below(3).min(self.next() - 1 - self.first);
        match dice.below(7) {
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
            2 => self.line(
                "model.called",
                Some(turn),
                KERNEL,
                None,
                &format!(
                    r#"{{"seen":{seen},"messages":1,"result":"error","error":{{"class":"compaction_paused","message":"放不下"}}}}"#
                ),
            ),
            3 => self.line(
                "tool.result",
                Some(turn),
                TOOL,
                None,
                r#"{"call_id":"call_5_1","status":"ok","blocks":[{"type":"text","text":"lib.rs"}],"duration_ms":12}"#,
            ),
            4 => self.line(
                "context.injected",
                Some(turn),
                KERNEL,
                None,
                r#"{"kind":"env","text":"<env/>"}"#,
            ),
            5 => self.line("ext.note", Some(turn), KERNEL, None, r#"{"x":[1]}"#),
            _ => self.say(dice),
        }
    }

    /// 回报、子会话回报、等的会话空了（不带回合编号）：下一轮可能照它接着开。
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
            self.start(dice, None);
        }
    }

    /// 撤几轮，交回撤的。偶尔撤一轮日志里没有的。
    fn revert(&mut self, dice: &mut Dice) -> Vec<Turn> {
        let turns = if dice.odds(300) {
            vec![(self.next() + 7, None)]
        } else if self.turns.is_empty() {
            return Vec::new();
        } else {
            let from = match dice.below(6) {
                0 => dice.below(self.turns.len() as u64) as usize,
                1 => self.turns.len().saturating_sub(3),
                _ => self.turns.len() - 1,
            };
            self.turns.split_off(from)
        };
        let numbers: Vec<u64> = turns.iter().map(|turn| turn.0).collect();
        self.line(
            "turn.reverted",
            None,
            PERSON,
            None,
            &format!(r#"{{"turns":{numbers:?}}}"#),
        );
        self.undone.push(turns.clone());
        turns
    }

    /// 重做（编辑重发）：撤掉最后一轮，开它的人话同一个命令重发（字可能换了），再开一轮。
    fn redo(&mut self, dice: &mut Dice) {
        let Some(&(turn, Some((_, cause)))) = self.turns.last() else {
            return;
        };
        let numbers = vec![turn];
        self.turns.pop();
        self.line(
            "turn.reverted",
            None,
            PERSON,
            None,
            &format!(r#"{{"turns":{numbers:?}}}"#),
        );
        let seq = self.next();
        let body = Script::words(dice);
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
        self.line(
            "message.withdrawn",
            self.open,
            KERNEL,
            None,
            &format!(r#"{{"messages":{picked:?}}}"#),
        );
    }
}

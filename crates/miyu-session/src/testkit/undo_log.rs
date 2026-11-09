//! 照种子造撤销、恢复的对照测试用的会话日志（施工 V-41 中：撤销的回应、撤销以后重算的她看过的文件，新旧读法比）。造出来的
//! 过得了账本（[`UndoLog::ledger`] 查）：主人闲着时说的（自己开一轮）、回合里排着的、斜杠命令模板展开的、多行的、只有图的，
//! 别的来路的话；回合里她的回复（带工具调用）和请求、工具结果（读、改、移进回收站的文件，派出去的后台命令和子代理，给子代理
//! 留的言）、不认识的种类；后台命令的回报、子代理的回报（做完、停掉、撤销时停掉、崩了补的）；撤销（最近一轮、最近几轮、
//! 很早的一轮）、恢复（连着恢复几次）、重做（撤掉最后一轮、同一个命令重发再开一轮）、撤回排着的话、压缩和清空（撤销跨压缩）；
//! 手动压缩单开的、照回报开的、不带触发的回合。分叉出来的从 `upto + 1` 数起，偶尔撤到分叉点以前的回合（这样的过不了只看
//! 自己的账本，[`UndoLog::ledger`] 不查）。
//!
//! 写成真文件时照种子分几段（[`UndoLog::write`]）。

use std::collections::BTreeMap;
use std::path::Path;

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;
use miyu_kernel::ledger::Ledger;

mod turn;

/// 照种子走的随机数（xorshift，同判官的对照测试）。
#[derive(Debug, Clone)]
pub struct Dice(pub u64);

impl Dice {
    /// `0..n` 里的一个。
    pub fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }

    /// `n` 回里有一回。
    pub fn odds(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

const PERSON: &str = r#"{"kind":"person"}"#;
const VIA: &str = r#"{"kind":"person","via":"qq:10001"}"#;
const OTHERS: [&str; 5] = [
    r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#,
    r#"{"kind":"module","id":"goal"}"#,
    r#"{"kind":"harness","name":"claude"}"#,
    r#"{"kind":"schedule","schedule":"t1"}"#,
    r#"{"kind":"kernel"}"#,
];
const KERNEL: &str = r#"{"kind":"kernel"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const POLICY: &str = r#""policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"full","read_only":false}"#;
const PARENT: &str = "01a0cf12-7e40-7d1b-8c3a-51d49f03b21c";
/// 文件：几个常用的，看过的、改过的会撞上。
const PATHS: [&str; 6] = [
    "/w/src/lib.rs",
    "/w/src/main.rs",
    "/w/README.md",
    "/w/a b/空格.txt",
    "/w/src/deep/mod.rs",
    "/w/Cargo.toml",
];

/// 派出去的一个任务。
#[derive(Debug, Clone)]
enum Job {
    /// 后台命令：报过结束了没有。
    Command { ended: bool },
    /// 子代理：子会话的编号（第几个），被停掉了没有。
    Agent { session: u64, stopped: bool },
}

/// 一次能恢复的撤销：撤的几轮，跟着撤掉的压缩（所在的回合、替代到哪）。
type Undone = (Vec<u64>, Vec<(u64, u64)>);

/// 造好的一段日志。
#[derive(Debug, Clone)]
pub struct UndoLog {
    /// 一行一条，照先后。
    pub lines: Vec<String>,
    /// 第一条的序号：分叉出来的是分叉点的下一条。
    pub first: u64,
    /// 撤销、恢复的那几条的序号（撤销的回应照它们写）。
    pub undos: Vec<u64>,
    /// 撤过分叉点以前的回合（只看自己的账本过不了）。
    pub reached_base: bool,
    /// 还没撤掉的回合，照先后（分叉出去时子会话接着用）。
    pub turns: Vec<u64>,
    /// 正在跑的一轮。
    open: Option<u64>,
    /// 这一轮里还没有结果的调用。
    pending: Vec<String>,
    /// 这一轮里排着的人话，还没被请求看到。
    queued: Vec<u64>,
    /// 上一条回复的序号。
    last_reply: Option<u64>,
    /// 还算数的压缩：所在的回合、替代到哪。
    compactions: Vec<(u64, u64)>,
    /// 能恢复的撤销：撤的几轮、跟着撤掉的压缩。
    undone: Vec<Undone>,
    /// 主人说过的话：序号、命令编号。
    said: Vec<(u64, u64)>,
    /// 回报：序号。
    reports: Vec<u64>,
    /// 派出去过的任务，照编号的数。
    jobs: BTreeMap<u64, Job>,
    /// 命令编号用到第几个。
    cause: u64,
    /// 分叉点以前还没撤掉的回合（分叉出来的才有）。
    base: Vec<u64>,
}

impl UndoLog {
    /// 照种子写一段：`steps` 步。`forked` 是分叉点和那时父会话还没撤掉的回合。
    pub fn run(dice: &mut Dice, steps: u64, forked: Option<(u64, Vec<u64>)>) -> UndoLog {
        let first = forked.as_ref().map_or(1, |(upto, _)| upto + 1);
        let mut log = UndoLog {
            lines: Vec::new(),
            first,
            undos: Vec::new(),
            reached_base: false,
            turns: Vec::new(),
            open: None,
            pending: Vec::new(),
            queued: Vec::new(),
            last_reply: None,
            compactions: Vec::new(),
            undone: Vec::new(),
            said: Vec::new(),
            reports: Vec::new(),
            jobs: BTreeMap::new(),
            cause: 0,
            base: forked
                .as_ref()
                .map(|(_, turns)| turns.clone())
                .unwrap_or_default(),
        };
        let created = match &forked {
            Some((upto, _)) => format!(
                r#"{{"venue":"local",{POLICY},"forked":{{"from":"{PARENT}","upto":{upto}}}}}"#
            ),
            None => format!(r#"{{"venue":"local",{POLICY}}}"#),
        };
        log.line("session.created", None, PERSON, None, &created);
        for _ in 0..steps {
            log.step(dice);
        }
        log
    }

    /// 过一遍账本（分叉出来的从第一条起的账本）：交回过不了的那一条的序号和原因。撤过分叉点以前的回合的不查，交回空的。
    pub fn ledger(&self) -> Option<String> {
        if self.reached_base {
            return None;
        }
        let mut ledger = Ledger::default();
        if let Some(first) = Seq::new(self.first).filter(|_| self.first > 1) {
            ledger = ledger.starting_at(first);
        }
        for line in &self.lines {
            let event = match Event::from_line(line) {
                Ok(event) => event,
                Err(error) => return Some(format!("{line}: {error}")),
            };
            if let Err(error) = ledger.append(&event) {
                return Some(error.to_string());
            }
        }
        None
    }

    /// 写成真文件：照种子分一到四段（段名是这一段第一条的序号），`lines` 是要写的行（可以是改过的）。偶尔最后多一个空段。
    ///
    /// # Panics
    ///
    /// 写不进去。
    pub fn write(dir: &Path, first: u64, lines: &[String], dice: &mut Dice) {
        std::fs::create_dir_all(dir).expect("建目录");
        let parts = 1 + dice.below(4) as usize;
        let mut cuts: Vec<usize> = (1..parts)
            .map(|_| 1 + dice.below(lines.len().max(2) as u64 - 1) as usize)
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        let mut start = 0;
        for end in cuts.into_iter().chain([lines.len()]) {
            if end <= start {
                continue;
            }
            let text: String = lines[start..end]
                .iter()
                .map(|line| format!("{line}\n"))
                .collect();
            let name = format!("{:012}.jsonl", first + start as u64);
            std::fs::write(dir.join(name), text).expect("写段");
            start = end;
        }
        if dice.odds(5) {
            let name = format!("{:012}.jsonl", first + lines.len() as u64);
            std::fs::write(dir.join(name), "").expect("写空段");
        }
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

    fn step(&mut self, dice: &mut Dice) {
        match (dice.below(24), self.open) {
            (0..=3, _) => self.say(dice),
            (4, _) => self.other(dice),
            (5..=6, None) => self.start(dice, None),
            (5..=12, Some(turn)) => self.inside(dice, turn),
            (13..=14, Some(turn)) => self.end(dice, turn),
            (15..=16, None) => self.revert(dice),
            (17, None) => self.unrevert(),
            (18, None) => self.redo(dice),
            (19, Some(turn)) => self.withdraw(dice, turn),
            (20, Some(turn)) => self.compact(dice, turn),
            (21..=22, _) => self.report(dice),
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
            4 => r#"{"blocks":[{"type":"text","text":"   "}]}"#.to_string(),
            _ => format!(
                r#"{{"blocks":[{{"type":"text","text":"问 {}"}}]}}"#,
                dice.below(1000)
            ),
        }
    }

    /// 主人说一句：在一轮里的排着，闲着的自己开一轮（偶尔不开）。
    fn say(&mut self, dice: &mut Dice) {
        self.cause += 1;
        let cause = self.cause;
        let seq = self.next();
        let body = UndoLog::words(dice);
        let by = if dice.odds(6) { VIA } else { PERSON };
        let turn = self.open;
        self.line("message.user", turn, by, Some(cause), &body);
        self.said.push((seq, cause));
        if turn.is_some() {
            self.queued.push(seq);
        } else if !dice.odds(5) {
            self.start(dice, Some(seq));
        }
    }

    /// 别的来路的一句。
    fn other(&mut self, dice: &mut Dice) {
        let by = OTHERS[dice.below(OTHERS.len() as u64) as usize];
        self.cause += 1;
        let turn = self.open.filter(|_| dice.odds(2));
        let seq = self.next();
        self.line(
            "message.user",
            turn,
            by,
            Some(self.cause),
            r#"{"blocks":[{"type":"text","text":"别处来的"}]}"#,
        );
        if turn.is_some() {
            self.queued.push(seq);
        } else if self.open.is_none() && dice.odds(3) {
            self.start(dice, Some(seq));
        }
    }

    /// 开一轮：照 `by`、回报、以前说过的话开的，不带触发的（手动压缩单开的那种）。
    fn start(&mut self, dice: &mut Dice, by: Option<u64>) {
        let seq = self.next();
        let trigger = by.or_else(|| match dice.below(4) {
            0 => self.reports.last().copied(),
            1 => self.said.last().map(|said| said.0),
            2 if !self.said.is_empty() => {
                Some(self.said[dice.below(self.said.len() as u64) as usize].0)
            }
            _ => None,
        });
        let body = trigger.map_or("{}".to_string(), |trigger| {
            format!(r#"{{"trigger":{trigger}}}"#)
        });
        self.undone.clear();
        self.line("turn.started", Some(seq), KERNEL, None, &body);
        self.open = Some(seq);
        self.turns.push(seq);
        self.queued.clear();
    }
}

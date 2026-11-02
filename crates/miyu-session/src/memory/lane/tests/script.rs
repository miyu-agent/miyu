//! 照种子造会话日志（施工 V-37 下的对照测试用）：人话（本机的人、平台上的人、不是人的）、开回合（照一句开、照记下的几句开、
//! 不带触发的）、并进来的、她答（有字、没字、调搜记忆、调别的）、调用的结果（列了记忆、没列、失败的）、注入的记忆块、结束（几种
//! 原因）、撤销、恢复、中途换人格、不在这一轮的回复。一行一条，序号接着往下数。

/// 照种子走的随机数（xorshift）。
pub(super) struct Dice(pub(super) u64);

impl Dice {
    /// `0..n` 里的一个。
    pub(super) fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }

    /// 一半一半。
    pub(super) fn coin(&mut self) -> bool {
        self.below(2) == 0
    }
}

const ALICE: &str = r#"{"kind":"person"}"#;
const BOB: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:10086"}"#;
const CAROL: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:10087"}"#;
const GOAL: &str = r#"{"kind":"module","id":"goal"}"#;
const MEMORY: &str = r#"{"kind":"module","id":"memory"}"#;
const KNOWLEDGE: &str = r#"{"kind":"module","id":"knowledge"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;

/// 说的话：有长有短，有空的。
const WORDS: [&str; 8] = [
    "周末去爬山",
    "",
    "带上水和伞",
    "团子吃什么",
    "猫粮加一点鸡胸肉",
    "阿杰下周六搬家，搬去张江，离公司近",
    "好",
    "我养了一只猫，叫团子，三岁了",
];

/// 造日志的一路：记着还没触发回合的人话、正在进行的那一轮、结束了的、撤掉了的、调了还没结果的搜索。克隆出来的接着各写各的
/// （分叉以后父会话、支线各往下说）。
#[derive(Clone)]
pub(super) struct Script {
    /// 下一条的序号。
    next: u64,
    pending: Vec<u64>,
    open: Option<u64>,
    ended: Vec<u64>,
    reverted: Vec<Vec<u64>>,
    calls: Vec<String>,
}

impl Script {
    /// 从第 `first` 条写起。
    pub(super) fn new(first: u64) -> Script {
        Script {
            next: first,
            pending: Vec::new(),
            open: None,
            ended: Vec::new(),
            reverted: Vec::new(),
            calls: Vec::new(),
        }
    }

    /// 下一条的序号。
    pub(super) fn next(&self) -> u64 {
        self.next
    }

    /// 写一条：种类、回合、谁、正文，交回这一行（不带换行）。时刻照序号走，一条一秒。
    fn line(&mut self, kind: &str, turn: Option<u64>, by: &str, body: &str) -> String {
        let seq = self.next;
        self.next += 1;
        let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
        format!(
            r#"{{"seq":{seq},"at":"2026-11-03T{:02}:{:02}:{:02}.000Z","kind":"{kind}"{turn},"by":{by},"body":{body}}}"#,
            seq / 3600 % 24,
            seq / 60 % 60,
            seq % 60
        )
    }

    /// 照种子往下写 `count` 条左右（一步可能写不出，跳过）。
    pub(super) fn run(&mut self, dice: &mut Dice, count: usize) -> Vec<String> {
        let mut lines = Vec::new();
        while lines.len() < count {
            if let Some(line) = self.step(dice) {
                lines.push(line);
            }
        }
        lines
    }

    /// 恢复最近一次撤掉的那几轮；没有撤掉的先撤一轮（结束了的一轮都没有的写不出）。
    pub(super) fn restore(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.reverted.is_empty() {
            let Some(turn) = self.ended.pop() else {
                return lines;
            };
            self.reverted.push(vec![turn]);
            lines.push(self.line(
                "turn.reverted",
                None,
                ALICE,
                &format!(r#"{{"turns":[{turn}]}}"#),
            ));
        }
        if let Some(turns) = self.reverted.pop() {
            lines.push(self.line(
                "turn.unreverted",
                None,
                ALICE,
                &format!(r#"{{"turns":{turns:?}}}"#),
            ));
            self.ended.extend(turns);
        }
        lines
    }

    /// 换成人格 `persona`（空的是无人格）。
    pub(super) fn recast(&mut self, persona: &str) -> String {
        self.line(
            "session.policy_changed",
            None,
            KERNEL,
            &format!(r#"{{"persona":"{persona}"}}"#),
        )
    }

    /// 一步。
    fn step(&mut self, dice: &mut Dice) -> Option<String> {
        match dice.below(100) {
            0..=24 => Some(self.say(dice)),
            25..=39 => self.start(dice),
            40..=44 => self.join(dice),
            45..=64 => self.reply(dice),
            65..=71 => self.result(dice),
            72..=75 => Some(self.inject(dice)),
            76..=87 => self.end(dice),
            88..=91 => self.revert(dice),
            92..=95 => self.unrevert(),
            96..=97 => {
                let persona = ["a", "b", ""][usize::try_from(dice.below(3)).unwrap_or(0)];
                Some(self.recast(persona))
            }
            _ => self.stray(dice),
        }
    }

    fn say(&mut self, dice: &mut Dice) -> String {
        let by = [ALICE, BOB, CAROL, GOAL][usize::try_from(dice.below(4)).unwrap_or(0)];
        let words = WORDS[usize::try_from(dice.below(8)).unwrap_or(0)];
        let body = match dice.below(6) {
            0 => r#"{"blocks":[]}"#.to_string(),
            _ => format!(r#"{{"blocks":[{{"type":"text","text":"{words}"}}]}}"#),
        };
        self.pending.push(self.next);
        self.line("message.user", None, by, &body)
    }

    fn start(&mut self, dice: &mut Dice) -> Option<String> {
        if self.open.is_some() {
            return None;
        }
        let turn = self.next;
        let body = match (dice.below(4), self.pending.last()) {
            (0, _) | (_, None) => "{}".to_string(),
            (1, Some(_)) => {
                let picked: Vec<u64> = self
                    .pending
                    .iter()
                    .copied()
                    .filter(|_| dice.coin())
                    .collect();
                match picked.last() {
                    Some(last) => format!(r#"{{"trigger":{last},"triggers":{picked:?}}}"#),
                    None => "{}".to_string(),
                }
            }
            (_, Some(_)) => {
                let at = usize::try_from(dice.below(self.pending.len() as u64)).unwrap_or(0);
                format!(r#"{{"trigger":{}}}"#, self.pending[at])
            }
        };
        if dice.below(5) > 0 {
            self.pending.clear();
        }
        self.open = Some(turn);
        Some(self.line("turn.started", Some(turn), KERNEL, &body))
    }

    fn join(&mut self, dice: &mut Dice) -> Option<String> {
        let turn = self.open?;
        let picked: Vec<u64> = self
            .pending
            .iter()
            .copied()
            .filter(|_| dice.coin())
            .collect();
        if picked.is_empty() {
            return None;
        }
        self.pending.retain(|seq| !picked.contains(seq));
        Some(self.line(
            "turn.joined",
            Some(turn),
            KERNEL,
            &format!(r#"{{"triggers":{picked:?}}}"#),
        ))
    }

    fn reply(&mut self, dice: &mut Dice) -> Option<String> {
        let turn = self.open?;
        let seen = self.next - 1;
        let block = match dice.below(5) {
            0 => r#"{"type":"text","text":""}"#.to_string(),
            1 | 2 => {
                let words = WORDS[usize::try_from(dice.below(8)).unwrap_or(0)];
                format!(r#"{{"type":"text","text":"好，{words}"}}"#)
            }
            3 => {
                let call = format!("call_{}_1", self.next);
                self.calls.push(call.clone());
                format!(
                    r#"{{"type":"tool_call","call_id":"{call}","name":"memory_search","args":"{{}}"}}"#
                )
            }
            _ => format!(
                r#"{{"type":"tool_call","call_id":"call_{}_2","name":"read","args":"{{}}"}}"#,
                self.next
            ),
        };
        Some(self.line(
            "message.assistant",
            Some(turn),
            MODEL,
            &format!(r#"{{"blocks":[{block}],"seen":{seen}}}"#),
        ))
    }

    fn result(&mut self, dice: &mut Dice) -> Option<String> {
        if self.calls.is_empty() {
            return None;
        }
        let at = usize::try_from(dice.below(self.calls.len() as u64)).unwrap_or(0);
        let call = self.calls.remove(at);
        let status = if dice.below(4) == 0 { "error" } else { "ok" };
        let text = match dice.below(3) {
            0 => "Nothing found.".to_string(),
            _ => format!(
                "m{} user 2026-10-08: 团子三岁\\nm{} user 2026-10-09: 爬山\\nm{} user 2026-10-09: 爬山\\n以前的对话：",
                1 + dice.below(5),
                1 + dice.below(5),
                1 + dice.below(5)
            ),
        };
        let by = format!(r#"{{"kind":"tool","call_id":"{call}"}}"#);
        let turn = self.open;
        Some(self.line(
            "tool.result",
            turn,
            &by,
            &format!(
                r#"{{"call_id":"{call}","status":"{status}","blocks":[{{"type":"text","text":"{text}"}}]}}"#
            ),
        ))
    }

    fn inject(&mut self, dice: &mut Dice) -> String {
        let by = if dice.below(3) == 0 {
            KNOWLEDGE
        } else {
            MEMORY
        };
        let refs: Vec<String> = (0..dice.below(4))
            .map(|_| format!("m{}", 1 + dice.below(6)))
            .collect();
        let refs = serde_json::to_string(&refs).unwrap_or_default();
        let turn = self.open;
        self.line(
            "context.injected",
            turn,
            by,
            &format!(r#"{{"kind":"memory","text":"<memories>\n</memories>","refs":{refs}}}"#),
        )
    }

    fn end(&mut self, dice: &mut Dice) -> Option<String> {
        let turn = self.open.take()?;
        self.ended.push(turn);
        let reason = ["completed", "completed", "interrupted", "error"]
            [usize::try_from(dice.below(4)).unwrap_or(0)];
        Some(self.line(
            "turn.ended",
            Some(turn),
            KERNEL,
            &format!(r#"{{"reason":"{reason}"}}"#),
        ))
    }

    fn revert(&mut self, dice: &mut Dice) -> Option<String> {
        if self.open.is_some() || self.ended.is_empty() {
            return None;
        }
        let count = 1 + usize::try_from(dice.below(2)).unwrap_or(0);
        let from = self.ended.len().saturating_sub(count);
        let turns: Vec<u64> = self.ended.split_off(from);
        self.reverted.push(turns.clone());
        Some(self.line(
            "turn.reverted",
            None,
            ALICE,
            &format!(r#"{{"turns":{turns:?}}}"#),
        ))
    }

    fn unrevert(&mut self) -> Option<String> {
        if self.open.is_some() {
            return None;
        }
        let turns = self.reverted.pop()?;
        self.ended.extend(turns.iter().copied());
        Some(self.line(
            "turn.unreverted",
            None,
            ALICE,
            &format!(r#"{{"turns":{turns:?}}}"#),
        ))
    }

    /// 不在这一轮的回复（照一轮结束了的编号写）。
    fn stray(&mut self, dice: &mut Dice) -> Option<String> {
        let at = usize::try_from(dice.below(self.ended.len() as u64)).ok()?;
        let turn = *self.ended.get(at)?;
        let seen = self.next - 1;
        Some(self.line(
            "message.assistant",
            Some(turn),
            MODEL,
            &format!(r#"{{"blocks":[{{"type":"text","text":"晚了的一句"}}],"seen":{seen}}}"#),
        ))
    }
}

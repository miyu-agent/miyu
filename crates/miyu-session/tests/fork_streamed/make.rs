//! 照种子造父会话的日志（施工 V-39 中的对照测试用）：每一条先交给父会话的账本，过得了才留，日志一定守规矩。人话（本机的人、
//! 平台上的人；回合当中来的排着）、开一轮（照一句开、带工作目录和加进来的目录）、她答（有字、调工具）、调用的结果、结束、
//! 压缩、撤销（连带压缩以前的）、恢复、撤回排着的话、换权限、换人格、换模型、换快照（有的、取不出的）、换工作目录。调用的
//! 结果三条里一条带读了文件的效果（施工 V-39 下）。

use miyu_kernel::event::Event;
use miyu_kernel::id::SessionId;
use miyu_kernel::ledger::Ledger;

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

    /// 挑一个。
    pub(super) fn pick<'a, T>(&mut self, from: &'a [T]) -> Option<&'a T> {
        from.get(usize::try_from(self.below(from.len() as u64)).ok()?)
    }
}

const PERSON: &str = r#"{"kind":"person"}"#;
const BOB: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:10086"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;
const DIRS: [&str; 3] = [r#"["/w/a"]"#, r#"["/w/a","/w/b"]"#, "[]"];
const CWDS: [&str; 3] = ["/w/one", "/w/two", "/w/three"];
/// 读了的文件的哈希（随便一份）。
const READ: &str = "sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb";

/// 造着的一份：账本、已经留下的行。
pub(super) struct Make {
    ledger: Ledger,
    pub(super) lines: Vec<String>,
    /// 说过的人话、开过的回合的序号。
    users: Vec<u64>,
    turns: Vec<u64>,
    /// 换快照时用的：取得出的那一份。
    policy: String,
}

impl Make {
    /// 第一条是 `created`（已经是一行，序号 1）。
    pub(super) fn new(parent: &SessionId, created: String, policy: String) -> Make {
        let mut ledger = Ledger::for_session(parent.clone());
        let event = Event::from_line(&created).expect("测试里造得出");
        // 第一条不是造会话的那几份，账本不认，后面的一条也写不进：照样造，分叉时报错。
        let _first = ledger.append(&event).is_ok();
        Make {
            ledger,
            lines: vec![created],
            users: Vec::new(),
            turns: Vec::new(),
            policy,
        }
    }

    /// 往下写 `count` 步（过不了账本的那一步不留）。
    pub(super) fn run(&mut self, dice: &mut Dice, count: usize) {
        for _ in 0..count {
            self.step(dice);
        }
    }

    /// 写一条：过得了账本才留。
    fn put(&mut self, kind: &str, turn: Option<u64>, by: &str, body: &str) -> bool {
        let seq = self.ledger.next_seq().get();
        let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
        let line = format!(
            r#"{{"seq":{seq},"at":"2026-11-03T08:{:02}:{:02}.000Z","kind":"{kind}"{turn},"by":{by},"body":{body}}}"#,
            seq / 60 % 60,
            seq % 60
        );
        let event = Event::from_line(&line).unwrap_or_else(|e| panic!("{line}：{e}"));
        if self.ledger.append(&event).is_err() {
            return false;
        }
        self.lines.push(line);
        true
    }

    fn step(&mut self, dice: &mut Dice) {
        let open = self.ledger.open_turn().map(|turn| turn.started().get());
        let next = self.ledger.next_seq().get();
        // 一步的调用多半先都有了结果，再请求下一步、结束这一轮。
        let waiting = !self.ledger.pending_calls().is_empty();
        let roll = match waiting && dice.below(4) > 0 {
            true => 52,
            false => dice.below(100),
        };
        match (roll, open) {
            (0..=21, _) => {
                let by = if dice.coin() { PERSON } else { BOB };
                let turn = open.filter(|_| dice.coin());
                let body = format!(r#"{{"blocks":[{{"type":"text","text":"第 {next} 句"}}]}}"#);
                if self.put("message.user", turn, by, &body) {
                    self.users.push(next);
                }
            }
            (22..=33, None) => {
                let trigger = match dice.pick(&self.users) {
                    Some(user) if dice.below(5) > 0 => format!(r#""trigger":{user}"#),
                    _ => String::new(),
                };
                let mut body = vec![trigger];
                if dice.coin() {
                    body.push(format!(
                        r#""cwd":"{}""#,
                        dice.pick(&CWDS).expect("测试里造得出")
                    ));
                }
                if dice.coin() {
                    body.push(format!(
                        r#""dirs":{}"#,
                        dice.pick(&DIRS).expect("测试里造得出")
                    ));
                }
                body.retain(|part| !part.is_empty());
                if self.put(
                    "turn.started",
                    Some(next),
                    KERNEL,
                    &format!("{{{}}}", body.join(",")),
                ) {
                    self.turns.push(next);
                }
            }
            (34..=51, Some(turn)) => {
                let blocks = match dice.below(3) {
                    0 => r#"{"type":"text","text":"好"}"#.to_string(),
                    1 => format!(
                        r#"{{"type":"tool_call","call_id":"call_{next}_1","name":"read","args":"{{}}"}}"#
                    ),
                    _ => format!(
                        r#"{{"type":"text","text":"看看"}},{{"type":"tool_call","call_id":"call_{next}_1","name":"read","args":"{{}}"}},{{"type":"tool_call","call_id":"call_{next}_2","name":"subagent","args":"{{}}"}}"#
                    ),
                };
                let body = format!(r#"{{"blocks":[{blocks}],"seen":{}}}"#, next - 1);
                self.put("message.assistant", Some(turn), MODEL, &body);
            }
            (52..=63, Some(turn)) => {
                let pending = self.ledger.pending_calls();
                if let Some(call) = dice.pick(&pending) {
                    let by = format!(r#"{{"kind":"tool","call_id":"{call}"}}"#);
                    // 三条里一条带读了文件的效果（施工 V-39 下：载入照底子记她看过的文件）。
                    let effects = match next % 3 {
                        0 => format!(
                            r#","effects":[{{"kind":"file.read","path":"/w/f{}","hash":"{READ}"}}]"#,
                            next % 7
                        ),
                        _ => String::new(),
                    };
                    let body = format!(
                        r#"{{"call_id":"{call}","status":"ok","blocks":[{{"type":"text","text":"结果 {next}"}}]{effects}}}"#
                    );
                    self.put("tool.result", Some(turn), &by, &body);
                }
            }
            (64..=73, Some(turn)) => {
                let reason = *dice
                    .pick(&["completed", "interrupted", "error"])
                    .expect("测试里造得出");
                self.put(
                    "turn.ended",
                    Some(turn),
                    KERNEL,
                    &format!(r#"{{"reason":"{reason}"}}"#),
                );
            }
            (74..=78, Some(turn)) => {
                let floor = self.ledger.compacted().map_or(1, |upto| upto.get());
                let upto = floor + dice.below(next - floor);
                let body = format!(r#"{{"upto":{upto},"summary":"摘要 {next}","trigger":"auto"}}"#);
                self.put("context.compacted", Some(turn), KERNEL, &body);
            }
            (79..=82, None) => {
                let turns = dice
                    .pick(&self.turns)
                    .and_then(|turn| {
                        self.ledger.turns_from(miyu_kernel::id::TurnId::new(
                            miyu_kernel::id::Seq::new(*turn)?,
                        ))
                    })
                    .unwrap_or_default();
                let turns: Vec<u64> = turns.iter().map(|turn| turn.started().get()).collect();
                self.put(
                    "turn.reverted",
                    None,
                    PERSON,
                    &format!(r#"{{"turns":{turns:?}}}"#),
                );
            }
            (83..=85, None) => {
                if let Some(turns) = self.ledger.last_reverted() {
                    let turns: Vec<u64> = turns.iter().map(|turn| turn.started().get()).collect();
                    self.put(
                        "turn.unreverted",
                        None,
                        PERSON,
                        &format!(r#"{{"turns":{turns:?}}}"#),
                    );
                }
            }
            (86..=88, Some(_)) => {
                let queued = self.ledger.queued();
                if let Some(message) = dice.pick(&queued) {
                    let body = format!(r#"{{"messages":[{}]}}"#, message.get());
                    self.put("message.withdrawn", None, PERSON, &body);
                }
            }
            (89..=94, _) => {
                let body = match dice.below(8) {
                    0 => format!(
                        r#"{{"permission":{{"level":"{}","read_only":{}}}}}"#,
                        dice.pick(&["workspace", "full"]).expect("测试里造得出"),
                        dice.coin()
                    ),
                    1 => format!(
                        r#"{{"persona":"{}"}}"#,
                        dice.pick(&["a", "b", ""]).expect("测试里造得出")
                    ),
                    2 => r#"{"model":"deepseek/deepseek-v4"}"#.to_string(),
                    7 => format!(r#"{{"policy":"sha256:{}"}}"#, "0".repeat(64)),
                    _ => format!(r#"{{"policy":"{}"}}"#, self.policy),
                };
                self.put("session.policy_changed", None, PERSON, &body);
            }
            (95..=99, _) => {
                let dirs = match dice.coin() {
                    true => format!(r#","dirs":{}"#, dice.pick(&DIRS).expect("测试里造得出")),
                    false => String::new(),
                };
                let body = format!(
                    r#"{{"cwd":"{}"{dirs}}}"#,
                    dice.pick(&CWDS).expect("测试里造得出")
                );
                self.put("session.workspace_changed", None, PERSON, &body);
            }
            _ => {}
        }
    }
}

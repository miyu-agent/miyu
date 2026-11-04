//! 只解要的种类的行为差别（施工 V-49，`docs/construction/V-49-history只解要的种类.md`）：照种子造的会话写成真段文件、经存储读
//! （执行器、会话表交给 `history` 的就是这条路），一行写坏三样，新的（只解要的种类）和以前的读法（全解，`old.rs`）比：
//!
//! - 不要的种类（注入的事实、会话的标题、命令、审批、目标、待办、包的状态……，`model.called` 不解的那几格）正文合 JSON、不合那一种
//!   的写法：新的照读，交回的同那一行没坏时（也同以前的读法读没坏的）；以前的读法报读不了。2026-11-05 项目主人批接受的就是这一处。
//! - 外壳坏了（序号写成字、时刻不对、种类写成数、没有 `body`、两个 `body`、写到一半）：交回的一字不差（读不了的报的错也是）。
//! - 要的种类（人话、回复、工具结果、造会话、切权限、摘要、回合的开头结尾、撤销、恢复、撤回、回报，`model.called` 撤销看的那几格）
//!   正文不合写法：同上。
//!
//! 还有两样给对照测试（`same.rs`）用的：[`vary`] 把生成器造的不认识的种类（`ext.note`）多半换成 `history` 不要的各种（合写法
//! 的）；[`Lines`] 是切成几段交的原文行，以前的读法照它全解、新的照它只解要的种类。

use std::sync::Mutex;

use super::script::{Dice, Script};
use super::*;
use crate::history::collect::ONLY;

/// 几段原文行：`read` 每一行全解（以前的读法照它读），`read_range_only` 每一行照 `Event::from_line_only` 只解要的种类（新的读法
/// 照它读，同执行器那份走存储的 `read_range_only`）。两样各解一遍记着（照 `only` 认），之后每次读照记着的拷：一份会话要读几十次。
pub(super) struct Lines {
    lines: Vec<Vec<String>>,
    parsed: Mutex<Vec<Parsed>>,
}

/// 照 `only`（全解的是空的）解好的每一段。
type Parsed = (Option<Vec<String>>, Vec<Result<Vec<Event>, String>>);

impl Lines {
    /// 这几段。
    pub(super) fn new(lines: Vec<Vec<String>>) -> Lines {
        Lines {
            lines,
            parsed: Mutex::new(Vec::new()),
        }
    }

    /// 照先后一段段交第 `from` 到第 `upto` 条，每一行照 `only` 解（空的全解）；一段里有读不成的报它。
    fn each(
        &self,
        (from, upto): (Seq, Seq),
        only: Option<&[&str]>,
        each: &mut dyn FnMut(Vec<Event>) -> bool,
    ) -> Result<(), String> {
        let key = only.map(|only| only.iter().map(ToString::to_string).collect::<Vec<_>>());
        let mut parsed = self.parsed.lock().expect("没有别的线程拿着");
        if !parsed.iter().any(|(had, _)| *had == key) {
            let parse = |line: &str| match only {
                Some(only) => Event::from_line_only(line, only),
                None => Event::from_line(line),
            };
            let segments = self.lines.iter().map(|segment| {
                segment
                    .iter()
                    .map(|line| parse(line).map_err(|error| error.to_string()))
                    .collect()
            });
            parsed.push((key.clone(), segments.collect()));
        }
        let segments = parsed
            .iter()
            .find(|(had, _)| *had == key)
            .map(|(_, all)| all.clone());
        drop(parsed);
        for segment in segments.unwrap_or_default() {
            let mut events = segment?;
            let past = events.last().is_some_and(|event| event.seq >= upto);
            events.retain(|event| event.seq >= from && event.seq <= upto);
            if !events.is_empty() && !each(events) || past {
                break;
            }
        }
        Ok(())
    }
}

impl ReadLog for Lines {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        let all = (Seq::FIRST, Seq::new(u64::MAX).unwrap_or(Seq::FIRST));
        self.each(all, None, each)
    }

    fn read_range_only(
        &self,
        from: Seq,
        upto: Seq,
        only: &[&str],
        each: &mut dyn FnMut(Vec<Event>) -> bool,
    ) -> Result<(), String> {
        self.each((from, upto), Some(only), each)
    }
}

/// 平时跑几份；`MIYU_HISTORY_FULL=1` 跑 [`BROKEN_FULL`] 份。
const BROKEN: u64 = 12;
/// `MIYU_HISTORY_FULL=1` 跑几份。
const BROKEN_FULL: u64 = 1500;

/// `history` 不要、正文不解的种类，合写法的正文（照 `docs/designs/samples/events/`）。`model.called` 另造（[`vary`]）。
const UNWANTED: [(&str, &str); 10] = [
    ("session.meta_changed", r#"{"title":"整理 omega"}"#),
    ("command.ran", r#"{"text":"/omega","command":"omega"}"#),
    (
        "tool.approval_decided",
        r#"{"call_id":"call_67_1","decision":"deny","reason":"omega"}"#,
    ),
    (
        "question.answered",
        r#"{"call_id":"call_77_1","answers":[{"picked":["omega"]}]}"#,
    ),
    ("goal.set", r#"{"text":"omega","limit":30}"#),
    ("todos.cleared", "{}"),
    ("package.state", r#"{"package":"todowrite","state":{}}"#),
    (
        "context.compaction_paused",
        r#"{"reason":"failures","failures":3}"#,
    ),
    (
        "shell.ran",
        r#"{"message":1,"exit":0,"duration_ms":1,"text":"omega","omitted":0}"#,
    ),
    ("session.recapped", r#"{"text":"omega","upto":1}"#),
];

/// 要的种类：`history` 自己看的，和有效历史照种类认的（测试里照规矩另抄一份，不引 `collect.rs`、内核 `only.rs` 的表：那边少写了
/// 这里对不上）。
const WANTED: [&str; 14] = [
    "session.created",
    "session.policy_changed",
    "message.user",
    "message.assistant",
    "tool.result",
    "context.compacted",
    "turn.started",
    "turn.ended",
    "turn.reverted",
    "turn.unreverted",
    "message.withdrawn",
    "job.reported",
    "child.reported",
    "peer.idle",
];

/// 一行的种类（生成器写的行 `kind` 在第三格）。
fn kind(line: &str) -> &str {
    let rest = &line[line.find(r#","kind":""#).map_or(0, |at| at + 9)..];
    &rest[..rest.find('"').unwrap_or(0)]
}

/// 外壳和 `body` 分开：生成器写的行 `body` 在最后一格。
fn split(line: &str) -> (&str, &str) {
    let at = line.find(r#","body":"#).expect("有 body");
    (&line[..at], &line[at + 8..line.len() - 1])
}

/// 把不认识的种类（`ext.note`）多半换成 `history` 不要的各种，偶尔是暂停着没发出去的请求（撤销认排着的话不算它看到的）。
pub(super) fn vary(dice: &mut Dice, lines: &mut [String]) {
    for line in lines.iter_mut() {
        if kind(line) != "ext.note" || dice.odds(4) {
            continue;
        }
        let (shell, _) = split(line);
        let seq: u64 = shell[7..shell.find(',').unwrap_or(7)]
            .parse()
            .expect("序号");
        let (new, body) = match dice.below(UNWANTED.len() as u64 + 2) {
            k if (k as usize) < UNWANTED.len() => {
                let (kind, body) = UNWANTED[k as usize];
                (kind, body.to_string())
            }
            _ => (
                "model.called",
                format!(
                    r#"{{"seen":{},"messages":1,"usage":{{"uncached":1,"cache_read":0,"cache_write":0,"output":1}},"result":"error","error":{{"class":"compaction_paused","message":"paused"}}}}"#,
                    seq - 1
                ),
            ),
        };
        let shell = shell.replace(r#""kind":"ext.note""#, &format!(r#""kind":"{new}""#));
        *line = format!(r#"{shell},"body":{body}}}"#);
    }
}

/// 不要的种类的正文写坏（合 JSON、不合那一种的写法）：`model.called` 坏在不解的 `endpoint`，别的整个正文写成一个数。不是不要的
/// 种类（不认识的种类全解也照读）的没有。
fn unwanted(line: &str) -> Option<String> {
    let (shell, body) = split(line);
    match kind(line) {
        "model.called" => Some(format!(
            r#"{shell},"body":{{"endpoint":7,{}}}}}"#,
            &body[1..body.len() - 1]
        )),
        kind if WANTED.contains(&kind) || kind.starts_with("ext.") => None,
        _ => Some(format!(r#"{shell},"body":7}}"#)),
    }
}

/// 要的种类的正文写坏：整个正文写成一个数；`model.called` 坏在撤销看的 `seen`，回复的 `seen` 偶尔写成 0。
fn wanted_broken(dice: &mut Dice, line: &str) -> Option<String> {
    let (shell, body) = split(line);
    match kind(line) {
        "model.called" => Some(format!(
            r#"{shell},"body":{{"seen":"x",{}}}}}"#,
            &body[body.find(',').map_or(1, |at| at + 1)..body.len() - 1]
        )),
        "message.assistant" if dice.odds(3) => {
            let at = body.rfind(r#""seen":"#).expect("回复有 seen");
            let rest = &body[at + 7..];
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            Some(format!(
                r#"{shell},"body":{}"seen":0{}}}"#,
                &body[..at],
                &rest[digits..]
            ))
        }
        kind if WANTED.contains(&kind) => Some(format!(r#"{shell},"body":7}}"#)),
        _ => None,
    }
}

/// 外壳写坏：序号写成字、时刻不对、种类写成数、没有 `body`、两个 `body`、写到一半。
fn shell_broken(dice: &mut Dice, line: &str) -> String {
    let (shell, body) = split(line);
    match dice.below(6) {
        0 => {
            let comma = line.find(',').unwrap_or(7);
            format!(r#"{{"seq":"{}"{}"#, &line[7..comma], &line[comma..])
        }
        1 => line.replacen(r#""at":"2026-"#, r#""at":"2026x"#, 1),
        2 => line.replacen(&format!(r#""kind":"{}""#, kind(line)), r#""kind":7"#, 1),
        3 => format!("{shell}}}"),
        4 => format!(r#"{shell},"body":{body},"body":{{}}}}"#),
        _ => {
            let half = (0..=line.len() / 2)
                .rev()
                .find(|&at| line.is_char_boundary(at))
                .unwrap_or(0);
            line[..half].to_string()
        }
    }
}

/// 写坏一行的几样。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Broken {
    Unwanted,
    Shell,
    Wanted,
}

#[test]
fn only_unwanted_bodies_that_are_still_json_read_as_if_unbroken() {
    let texts = texts();
    let seeds = match std::env::var("MIYU_HISTORY_FULL").as_deref() {
        Ok("1") => BROKEN_FULL,
        _ => BROKEN,
    };
    let mut counts = std::collections::BTreeMap::<(Broken, bool), usize>::new();
    for seed in 0..seeds {
        let mut dice = Dice(seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1);
        let child = dice.odds(5);
        let forked = dice.odds(6).then(|| 3 + dice.below(400));
        let steps = 30 + dice.below(400);
        let mut script = Script::run(&mut dice, steps, child, forked);
        vary(&mut dice, &mut script.lines);
        let cuts: Vec<usize> = (0..dice.below(4))
            .map(|_| dice.below(script.lines.len() as u64) as usize)
            .collect();
        let offset = UtcOffset::from_minutes(*dice.pick(&[0, 540, -300])).expect("在范围里");
        let small = Tuning {
            flush_at: 2 + dice.below(60) as usize,
            keep: 1 + dice.below(16) as usize,
        };
        let clean = Scratch::new();
        write(&clean.0, &script.lines, script.first, &cuts, None, false);
        let clean = Log::new(Dir(clean.0.clone()));
        for how in [Broken::Unwanted, Broken::Shell, Broken::Wanted] {
            let mut lines = script.lines.clone();
            let picked: Vec<usize> = (0..lines.len())
                .filter(|&n| match how {
                    Broken::Unwanted => unwanted(&lines[n]).is_some(),
                    Broken::Shell => true,
                    Broken::Wanted => {
                        WANTED.contains(&kind(&lines[n])) || kind(&lines[n]) == "model.called"
                    }
                })
                .collect();
            // 要的种类里请求只占零头，三回里一回专挑它（撤销看的 `seen` 坏了的照报）。
            let calls: Vec<usize> = match how {
                Broken::Wanted if dice.odds(3) => picked
                    .iter()
                    .copied()
                    .filter(|&n| kind(&lines[n]) == "model.called")
                    .collect(),
                _ => Vec::new(),
            };
            let picked = if calls.is_empty() { picked } else { calls };
            if picked.is_empty() {
                continue;
            }
            let n = picked[dice.below(picked.len() as u64) as usize];
            let broken = match how {
                Broken::Unwanted => unwanted(&lines[n]),
                Broken::Shell => Some(shell_broken(&mut dice, &lines[n])),
                Broken::Wanted => wanted_broken(&mut dice, &lines[n]),
            };
            lines[n] = broken.expect("挑的是写得坏的");
            let scratch = Scratch::new();
            write(&scratch.0, &lines, script.first, &cuts, None, false);
            let log = Log::new(Dir(scratch.0.clone()));
            // 存储报的错（连为什么）也一样：交回的那一句只写哪一段第几行。
            if how != Broken::Unwanted {
                let full =
                    read_segments(&scratch.0, |_| true).map_err(|error| format!("{error:?}"));
                let mine = read_range_only(&scratch.0, Seq::FIRST, None, ONLY, |_| Flow::Go)
                    .map_err(|error| format!("{error:?}"));
                assert_eq!(mine, full, "seed {seed} {how:?} 第 {n} 行 {}", lines[n]);
            }
            for args in [
                json!({}),
                json!({"query": "omega"}),
                json!({"from": (script.first + n as u64).saturating_sub(3), "limit": 7}),
            ] {
                let call = call(args.clone(), None, offset.minutes());
                let parsed: Args = serde_json::from_value(args.clone()).expect("读得懂");
                let wanted = wanted(&texts, &call, parsed).unwrap_or_else(|_| panic!("{args}"));
                let old = old::look(&texts, &log, offset, &wanted, &call.stop);
                let expected = match how {
                    Broken::Unwanted => {
                        assert!(
                            old.error,
                            "seed {seed} 第 {n} 行以前的读法该报读不了：{}",
                            lines[n]
                        );
                        old::look(&texts, &clean, offset, &wanted, &call.stop)
                    }
                    _ => old.clone(),
                };
                *counts.entry((how, old.error)).or_default() += 1;
                for tuning in [small, Tuning::SHIPPED] {
                    let new = look_tuned(&texts, &log, offset, &wanted, &call.stop, tuning);
                    assert_eq!(
                        new, expected,
                        "seed {seed} {how:?} 第 {n} 行 {} {args} {tuning:?}",
                        lines[n]
                    );
                }
            }
        }
    }
    eprintln!("写坏的几样（哪样、以前的读法报没报）比了几次：{counts:?}");
    for how in [Broken::Unwanted, Broken::Shell, Broken::Wanted] {
        assert!(
            counts.get(&(how, true)).is_some_and(|&n| n > 0),
            "{how:?} 没比到报读不了的：{counts:?}"
        );
    }
}

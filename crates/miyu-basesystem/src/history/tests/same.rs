//! 新旧对照（施工 V-45）：以前的读法（整份读、整份交给留着一切的有效历史、整份挑、整份筛，照抄在 `same/old.rs`）当准绳，照种子
//! 造会话（`same/script.rs`），每一种参数（读、找，翻页、范围、时刻、谁说的、一页几条，越界的、什么都没有的）新旧交回的
//! `Done` 逐字节比。边读边放调小（逼出放早了再读一遍的）和出厂的各比一遍；日志切成几段交，几份写成真的段文件经存储读
//! （定位读、一块块交），坏一行的报的错也比。再查三条退路（再读一遍、骨架认不得整份读、调用编号对不上整份读）都走到。
//!
//! 新的读法每一行只解要的种类（施工 V-49）：切成几段交的照 `Event::from_line_only` 解（`only.rs` 的 `Lines`），真段文件经存储的
//! `read_range_only`；以前的读法照旧全解。只解要的种类那一处行为差别（不要的种类正文合 JSON、不合写法的不报）在 `only.rs` 比。
//!
//! 平时每次跑 [`SEEDS`] 份（Windows 的 debug 门禁一趟已经很长，见施工单）；`MIYU_HISTORY_FULL=1` 跑 [`FULL`] 份。

mod old;
mod only;
mod script;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::id::Seq;
use miyu_store::log::{Flow, read_range, read_range_only, read_segments};
use miyu_store::sweep::{Ended, Plan, Sweep, Tuning};

use super::*;
use crate::history::collect::Collect;
use crate::history::{Args, Texts, Wanted, look_tuned, wanted};
use only::{Lines, vary};
use script::{Dice, Script, WORDS};

/// 平时跑几份。
const SEEDS: u64 = 12;
/// `MIYU_HISTORY_FULL=1` 跑几份。
const FULL: u64 = 3000;

/// 出厂的字。
fn texts() -> Texts {
    let resources = resources();
    History::load(
        &resources,
        Common::load(&resources).expect("共用的字读得出来"),
    )
    .expect("history 的字读得出来")
    .texts
}

/// 会话目录，照存储读（同执行器交给调用的 `LogDir`）。
struct Dir(PathBuf);

impl ReadLog for Dir {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        read_segments(&self.0, each).map_err(|error| error.to_string())
    }

    fn read_range(
        &self,
        from: Seq,
        upto: Seq,
        each: &mut dyn FnMut(Vec<Event>) -> bool,
    ) -> Result<(), String> {
        read_range(&self.0, from, Some(upto), |events| match each(events) {
            true => Flow::Go,
            false => Flow::Stop,
        })
        .map_err(|error| error.to_string())
    }

    fn read_range_only(
        &self,
        from: Seq,
        upto: Seq,
        only: &[&str],
        each: &mut dyn FnMut(Vec<Event>) -> bool,
    ) -> Result<(), String> {
        read_range_only(&self.0, from, Some(upto), only, |events| {
            match each(events) {
                true => Flow::Go,
                false => Flow::Stop,
            }
        })
        .map_err(|error| error.to_string())
    }
}

/// 用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-history-same-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

/// 把 `lines`（第一条是第 `first` 条）写成几段真文件：`cuts` 是各段从第几行起；`broken` 的那一行写坏；`half` 接在最后一段末尾。
fn write(
    dir: &std::path::Path,
    lines: &[String],
    first: u64,
    cuts: &[usize],
    broken: Option<usize>,
    half: bool,
) {
    let mut starts = vec![0];
    starts.extend(
        cuts.iter()
            .copied()
            .filter(|&cut| cut > 0 && cut < lines.len()),
    );
    starts.sort_unstable();
    starts.dedup();
    for (k, &start) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(lines.len());
        let mut text = String::new();
        for (n, line) in lines.iter().enumerate().take(end).skip(start) {
            match broken == Some(n) {
                true => text.push_str("{\"seq\":"),
                false => text.push_str(line),
            }
            text.push('\n');
        }
        if half && end == lines.len() {
            text.push_str(r#"{"seq":"#);
        }
        let name = format!("{:012}.jsonl", first + start as u64);
        fs::write(dir.join(name), text).expect("写得进");
    }
}

/// 照种子挑几种参数：读、找，翻页、范围、时刻、谁说的、一页几条，越界的。
fn combos(dice: &mut Dice, events: &[Event], offset: UtcOffset) -> Vec<Value> {
    let first = events.first().map_or(1, |event| event.seq.get());
    let last = events.last().map_or(1, |event| event.seq.get());
    let seq = |dice: &mut Dice| first + dice.below(last - first + 1);
    let minute = |dice: &mut Dice| {
        let event = &events[dice.below(events.len() as u64) as usize];
        let at = event.at.local_minute(offset);
        match dice.odds(3) {
            true => at[..10].to_string(),
            false => at,
        }
    };
    let query = |dice: &mut Dice| -> String {
        match dice.below(6) {
            0 => "zzz".to_string(),
            1 => format!("{} {}", dice.pick(&WORDS), dice.pick(&WORDS)),
            2 => "OMEGA beta".to_string(),
            _ => dice.pick(&WORDS).to_string(),
        }
    };
    let by = ["user", "assistant", "tool"];
    let mut all = vec![
        json!({}),
        json!({"limit": 1 + dice.below(4)}),
        json!({"limit": 5000}),
        json!({"from": seq(dice)}),
        json!({"from": seq(dice), "limit": 1 + dice.below(30)}),
        json!({"from": seq(dice), "to": seq(dice)}),
        json!({"by": dice.pick(&by)}),
        json!({"since": minute(dice)}),
        json!({"until": minute(dice), "by": dice.pick(&by)}),
        json!({"from": last + 3}),
        json!({"query": query(dice)}),
        json!({"query": query(dice)}),
        json!({"query": "omega"}),
        json!({"query": query(dice), "limit": 1 + dice.below(5)}),
        json!({"query": query(dice), "limit": 4000}),
        json!({"query": query(dice), "to": seq(dice)}),
        json!({"query": query(dice), "from": seq(dice), "by": dice.pick(&by)}),
        json!({"query": query(dice), "since": minute(dice)}),
    ];
    // 几种里再随便挑几样，免得每份都跑满。
    all.retain(|_| !dice.odds(3));
    all
}

/// 新旧在 `log` 上照每一种参数比一遍；交回比了几次。
fn compare(
    texts: &Texts,
    log: &Log,
    dice: &mut Dice,
    events: &[Event],
    tunings: &[Tuning],
    what: &str,
) -> usize {
    let offset =
        UtcOffset::from_minutes(*dice.pick(&[0, 540, -300, 345, -720, 840])).expect("在范围里");
    let mut compared = 0;
    for args in combos(dice, events, offset) {
        let call = call(args.clone(), None, offset.minutes());
        let parsed: Args = serde_json::from_value(args.clone()).expect("读得懂");
        let Ok(wanted) = wanted(texts, &call, parsed) else {
            continue;
        };
        let old = old::look(texts, log, offset, &wanted, &call.stop);
        for &tuning in tunings {
            let new = look_tuned(texts, log, offset, &wanted, &call.stop, tuning);
            assert_eq!(new, old, "{what} {args} {tuning:?}");
            compared += 1;
        }
    }
    compared
}

/// 这一份照 `tuning` 读会走哪条路：收齐、再读一遍、骨架认不得、调用编号对不上。
fn path(texts: &Texts, events: &[Event], tuning: Tuning) -> &'static str {
    let wanted = Wanted {
        words: Vec::new(),
        from: 1,
        to: u64::MAX,
        since: None,
        until: None,
        by: None,
        limit: 20,
    };
    let mut sweep = Sweep::new(
        Plan::guess(),
        tuning,
        Collect::new(&texts.placeholders, &wanted, UtcOffset::UTC),
    );
    if matches!(sweep.feed(events.to_vec()), Flow::Stop) {
        return "stopped";
    }
    sweep.finish();
    match (sweep.ended, sweep.settled.bad()) {
        (Ended::Unsure, _) => "unsure",
        (_, true) => "bad",
        (Ended::Again, false) => "again",
        (Ended::Done, false) => "done",
    }
}

#[test]
fn the_new_reading_is_byte_for_byte_the_old_one() {
    let texts = texts();
    let seeds = match std::env::var("MIYU_HISTORY_FULL").as_deref() {
        Ok("1") => FULL,
        _ => SEEDS,
    };
    let mut paths = std::collections::BTreeMap::<&str, usize>::new();
    let mut compared = 0;
    for seed in 0..seeds {
        let mut dice = Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let steps = match dice.below(10) {
            0 => dice.below(4),
            1..=6 => 20 + dice.below(250),
            _ => 250 + dice.below(1500),
        };
        let child = dice.odds(5);
        let forked = dice.odds(6).then(|| 3 + dice.below(400));
        let mut script = Script::run(&mut dice, steps, child, forked);
        vary(&mut dice, &mut script.lines);
        let events: Vec<Event> = script
            .lines
            .iter()
            .map(|line| Event::from_line(line).expect("造的行读得懂"))
            .collect();
        let small = Tuning {
            flush_at: 2 + dice.below(60) as usize,
            keep: 1 + dice.below(16) as usize,
        };
        for tuning in [small, Tuning::SHIPPED] {
            *paths.entry(path(&texts, &events, tuning)).or_default() += 1;
        }
        let mut segments = Vec::new();
        let mut rest = script.lines.as_slice();
        while !rest.is_empty() {
            let take = (1 + dice.below(400) as usize).min(rest.len());
            segments.push(rest[..take].to_vec());
            rest = &rest[take..];
        }
        compared += compare(
            &texts,
            &Log::new(Lines::new(segments)),
            &mut dice,
            &events,
            &[small, Tuning::SHIPPED],
            &format!("seed {seed}"),
        );
        if seed % 8 == 0 && !events.is_empty() {
            let scratch = Scratch::new();
            let cuts: Vec<usize> = (0..dice.below(4))
                .map(|_| dice.below(events.len() as u64) as usize)
                .collect();
            let broken = dice
                .odds(4)
                .then(|| dice.below(events.len() as u64) as usize);
            write(
                &scratch.0,
                &script.lines,
                script.first,
                &cuts,
                broken,
                dice.odds(3),
            );
            let log = Log::new(Dir(scratch.0.clone()));
            compared += compare(
                &texts,
                &log,
                &mut dice,
                &events,
                &[small, Tuning::SHIPPED],
                &format!("seed {seed} files {cuts:?} {broken:?}"),
            );
        }
    }
    eprintln!("比了 {compared} 次，走的路 {paths:?}");
    if seeds >= SEEDS {
        for way in ["done", "again"] {
            assert!(
                paths.get(way).is_some_and(|&n| n > 0),
                "{way} 没走到：{paths:?}"
            );
        }
    }
    if seeds >= FULL {
        for way in ["unsure", "bad"] {
            assert!(
                paths.get(way).is_some_and(|&n| n > 0),
                "{way} 没走到：{paths:?}"
            );
        }
    }
}

/// 内核写不出、生成器难得造到的几种，照手写的日志比：撤一轮日志里没有的（骨架认不得，整份读）；回复里 `history` 调用的编号
/// 不是它自己的序号、结果在它前面（调用编号对不上规矩，整份读）。新旧一样，翻记录的那条结果照旧不算。
#[test]
fn logs_the_skeleton_or_call_ids_cannot_follow_read_the_same() {
    let texts = texts();
    let line = |seq: u64, kind: &str, turn: Option<u64>, by: &str, body: &str| {
        let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
        let at = script::at(seq);
        Event::from_line(&format!(
            r#"{{"seq":{seq},"at":"{at}","kind":"{kind}"{turn},"by":{by},"body":{body}}}"#
        ))
        .expect("手写的行读得懂")
    };
    let created = r#"{"venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"full","read_only":false}}"#;
    let kernel = r#"{"kind":"kernel"}"#;
    let person = r#"{"kind":"person"}"#;
    let model = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
    let result = |seq: u64, call: &str, text: &str| {
        let by = format!(r#"{{"kind":"tool","call_id":"{call}"}}"#);
        let body = format!(
            r#"{{"call_id":"{call}","status":"ok","blocks":[{{"type":"text","text":"{text}"}}]}}"#
        );
        line(seq, "tool.result", Some(3), &by, &body)
    };
    let lookup = |seq: u64, call: &str| {
        let body = format!(
            r#"{{"blocks":[{{"type":"text","text":"翻一下 blue"}},{{"type":"tool_call","call_id":"{call}","name":"history","args":"{{}}"}}],"seen":{}}}"#,
            seq - 1
        );
        line(seq, "message.assistant", Some(3), model, &body)
    };
    let head = || {
        vec![
            line(1, "session.created", None, person, created),
            line(
                2,
                "message.user",
                None,
                person,
                r#"{"blocks":[{"type":"text","text":"blue whale"}]}"#,
            ),
            line(3, "turn.started", Some(3), kernel, r#"{"trigger":2}"#),
        ]
    };
    // 撤一轮日志里没有的：骨架认不得。后面还有算数的几条。
    let mut unsure = head();
    unsure.push(line(
        4,
        "turn.ended",
        Some(3),
        kernel,
        r#"{"reason":"completed"}"#,
    ));
    unsure.push(line(5, "turn.reverted", None, person, r#"{"turns":[99]}"#));
    for seq in 6..30 {
        unsure.push(line(
            seq,
            "message.user",
            None,
            person,
            r#"{"blocks":[{"type":"text","text":"blue sky"}]}"#,
        ));
    }
    // 结果在前、回复在后，回复里的编号指着前面（不是它自己的序号）。
    let mut early = head();
    early.push(result(4, "call_2_1", "blue result"));
    early.push(lookup(5, "call_2_1"));
    // 结果的编号指着后面的回复。
    let mut ahead = head();
    ahead.push(result(4, "call_5_1", "blue result"));
    ahead.push(lookup(5, "call_5_1"));
    let tiny = Tuning {
        flush_at: 2,
        keep: 1,
    };
    for (what, events) in [("unsure", unsure), ("early", early), ("ahead", ahead)] {
        let expected = match what {
            "unsure" => "unsure",
            _ => "bad",
        };
        assert_eq!(path(&texts, &events, tiny), expected, "{what}");
        let log = log(vec![events.clone()]);
        for args in [json!({}), json!({"query": "blue"}), json!({"limit": 3})] {
            let call = call(args.clone(), None, 0);
            let parsed: Args = serde_json::from_value(args.clone()).expect("读得懂");
            let wanted = wanted(&texts, &call, parsed).unwrap_or_else(|_| panic!("{args}"));
            let old = old::look(&texts, &log, UtcOffset::UTC, &wanted, &call.stop);
            for tuning in [tiny, Tuning::SHIPPED] {
                let new = look_tuned(&texts, &log, UtcOffset::UTC, &wanted, &call.stop, tuning);
                assert_eq!(new, old, "{what} {args} {tuning:?}");
            }
            if what != "unsure" {
                assert!(
                    !text(&old).contains("blue result"),
                    "{what} {args}：{}",
                    text(&old)
                );
            }
        }
    }
}

/// 一页被字数撑满的（几百条不长不短的，没有一条就超过一页的）：「读」「找」都停在原来停的那一条。造的会话里每几十条就有一条
/// 比一页还长的，页多半停在它那里，估下限估多了也看不出来，这里单独造。
#[test]
fn pages_filled_by_middling_entries_stop_where_they_used_to() {
    let texts = texts();
    let mut dice = Dice(0x5EED);
    let mut events = vec![Event::from_line(r#"{"seq":1,"at":"2026-11-03T00:00:00.000Z","kind":"session.created","by":{"kind":"person"},"body":{"venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"full","read_only":false}}}"#).expect("读得懂")];
    for seq in 2..700 {
        let text = format!("omega {}", "字".repeat(30 + dice.below(300) as usize));
        let (by, venue) = match dice.odds(3) {
            true => (
                json!({"kind": "external", "venue": "qq:group:1", "id": "qq:1", "role": "member"}),
                json!({"msg": format!("m{seq}"), "name": "A"}),
            ),
            false => (json!({"kind": "person"}), Value::Null),
        };
        let mut body = json!({"blocks": [{"type": "text", "text": text}]});
        if !venue.is_null() {
            body["venue"] = venue;
        }
        let line = json!({"seq": seq, "at": script::at(seq), "kind": "message.user", "by": by, "body": body});
        events.push(Event::from_line(&line.to_string()).expect("读得懂"));
    }
    let log = log(vec![events]);
    for args in [
        json!({"limit": 5000}),
        json!({"from": 123, "limit": 5000}),
        json!({"query": "omega", "limit": 4000}),
        json!({"query": "OMEGA 字", "to": 400, "limit": 4000}),
    ] {
        let call = call(args.clone(), None, 0);
        let parsed: Args = serde_json::from_value(args.clone()).expect("读得懂");
        let wanted = wanted(&texts, &call, parsed).unwrap_or_else(|_| panic!("{args}"));
        let old = old::look(&texts, &log, UtcOffset::UTC, &wanted, &call.stop);
        assert!(
            text(&old).contains("Next") || text(&old).contains("Older"),
            "{args} 页是被撑满的"
        );
        for tuning in [
            Tuning {
                flush_at: 16,
                keep: 4,
            },
            Tuning::SHIPPED,
        ] {
            let new = look_tuned(&texts, &log, UtcOffset::UTC, &wanted, &call.stop, tuning);
            assert_eq!(new, old, "{args} {tuning:?}");
        }
    }
}

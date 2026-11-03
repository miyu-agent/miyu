//! 问话列表新旧对照（施工 SP-2）：准绳是整份 `read_events` → `History::whole` → 有效历史里主人本人的 `message.user` → 照会话
//! 表 `preview` 的取法取字（闲着时说的照它开的那一轮记回合）。照种子造几千份本机会话（`tests/script.rs`：撤销、恢复、重做、
//! 撤回、压缩、撤销跨压缩、分叉出来的、别的来路的话、回报接着开的、骨架认不得的），写成几段、末尾偶尔半行，放法多半调小
//! （逼出放早了再读一遍的），逐条比。坏行、序号断了、没有这个会话的，报错同整份读。

mod script;

use std::fs;
use std::path::{Path, PathBuf};

use miyu_kernel::event::Body;
use miyu_kernel::history::History;
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::origin::By;
use miyu_store::log::{OpenError, read_events, read_range_only};

use super::{Asked, CHARS, ONLY, Prompt, list, whole};
use crate::sweep::{Ended, Plan, Sweep, Tuning};
use crate::test_support::{TempDir, temp_root};
use script::{Dice, Script};

/// 准绳：整份读、整份交给留着一切的有效历史。
fn oracle(dir: &Path) -> Result<Vec<Prompt>, OpenError> {
    let mut history = History::whole();
    for event in read_events(dir)? {
        history.append(event);
    }
    let events = history.events();
    Ok(events
        .iter()
        .filter_map(|event| {
            let Body::MessageUser(message) = &event.body else {
                return None;
            };
            if !matches!(&event.by, By::Person(_)) {
                return None;
            }
            let turn = event.turn.or_else(|| {
                // 它开的那一轮在它后面（触发写成还没有的序号的那种，不算）。
                events.iter().find_map(|started| match &started.body {
                    Body::TurnStarted(body)
                        if body.trigger == Some(event.seq) && started.seq > event.seq =>
                    {
                        started.turn
                    }
                    _ => None,
                })
            });
            Some(Prompt {
                seq: event.seq,
                turn,
                at: event.at,
                text: miyu_store::index::said(message, CHARS),
            })
        })
        .collect())
}

/// 一个临时的会话目录。
fn session_dir(n: u64) -> (PathBuf, TempDir) {
    let (root, temp) = temp_root("asked");
    let id = SessionId::parse(&format!("01a0e000-0000-7000-8000-{n:012}")).expect("合写法");
    let dir = root.session_dir(&id);
    fs::create_dir_all(&dir).expect("建得了");
    (dir, temp)
}

/// 把 `lines` 写成几段：`cuts` 是各段从第几行（下标）起，段名照那一段第一条的序号；`half` 接在最后一段末尾。
fn write(dir: &Path, lines: &[String], first: u64, cuts: &[usize], half: &str) {
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
        for line in &lines[start..end] {
            text.push_str(line);
            text.push('\n');
        }
        if end == lines.len() {
            text.push_str(half);
        }
        fs::write(
            dir.join(format!("{:012}.jsonl", first + start as u64)),
            text,
        )
        .expect("写得了");
    }
}

/// 照种子的放法：多半放得勤、留得少（逼出放早了的），偶尔照出厂的。
fn tuning(seed: u64) -> Tuning {
    let mut dice = Dice(seed ^ 0x5DEE_CE66_D1CE_4E5B);
    if seed.is_multiple_of(7) {
        return Tuning::SHIPPED;
    }
    Tuning {
        flush_at: 1 + dice.below(24) as usize,
        keep: 1 + dice.below(10) as usize,
    }
}

/// 头一遍怎么样；放早了的，照下界再读的那一遍要收齐（不靠整份读兜底）。
fn first_pass(dir: &Path, tuning: Tuning) -> Ended {
    let mut sweep = Sweep::new(Plan::guess(), tuning, Asked::default());
    read_range_only(dir, Seq::FIRST, None, ONLY, |events| sweep.feed(events)).expect("读得出");
    if sweep.ended == Ended::Again {
        let mut again = Sweep::new(Plan::exact(sweep.reaches), tuning, Asked::default());
        read_range_only(dir, Seq::FIRST, sweep.last, ONLY, |events| {
            again.feed(events)
        })
        .expect("读得出");
        assert_eq!(
            again.ended,
            Ended::Done,
            "照下界再读的那一遍还放早了：{dir:?}"
        );
    }
    sweep.ended
}

/// 照种子造一份、写下来：交回目录和分叉点。
fn seeded(seed: u64) -> (PathBuf, TempDir, Script) {
    let mut dice = Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let steps = 10 + dice.below(320);
    let forked = dice.odds(8).then(|| 40 + dice.below(200));
    let script = Script::run(&mut dice, steps, forked);
    let (dir, temp) = session_dir(seed);
    let lines = &script.lines;
    let cuts: Vec<usize> = (0..dice.below(4))
        .map(|_| dice.below(lines.len() as u64) as usize)
        .collect();
    let half = match dice.below(4) {
        0 => r#"{"seq":999999,"kind":"message.user","by":{"kind":"pers"#,
        _ => "",
    };
    let first = forked.map_or(1, |upto| upto + 1);
    write(&dir, lines, first, &cuts, half);
    (dir, temp, script)
}

#[test]
fn the_same_as_the_whole_read_over_seeded_sessions() {
    let (mut again, mut unsure, mut reverted, mut listed) = (0, 0, 0, 0);
    for seed in 1..=3000 {
        let (dir, _temp, script) = seeded(seed);
        let tuning = tuning(seed);
        let new = list(&dir, tuning).map_err(|error| error.to_string());
        let old = oracle(&dir).map_err(|error| error.to_string());
        assert_eq!(new, old, "种子 {seed}");
        match first_pass(&dir, tuning) {
            Ended::Again => again += 1,
            Ended::Unsure => unsure += 1,
            Ended::Done => {}
        }
        let all = read_events(&dir).expect("读得出");
        let persons = all
            .iter()
            .filter(|event| {
                matches!(event.body, Body::MessageUser(_)) && matches!(&event.by, By::Person(_))
            })
            .count();
        reverted += usize::from(old.as_ref().map_or(0, Vec::len) != persons);
        listed += old.map_or(0, |old| old.len());
        drop(script);
    }
    assert!(again > 50, "放早了再读一遍的才 {again} 份");
    assert!(unsure > 3, "骨架认不得、整份读的才 {unsure} 份");
    assert!(
        reverted > 1000,
        "有效历史和全列出来不一样的才 {reverted} 份"
    );
    assert!(listed > 30_000, "一共才列了 {listed} 句");
}

/// 坏了的报坏了（哪一段、第几行同整份读），坏在后头的也报；序号断了的报坏了；一整行都还没写完的是空的、没有目录的是没有，同整份读。
#[test]
fn broken_logs_are_refused_like_the_whole_read() {
    let script = Script::run(&mut Dice(12_345), 200, None);
    let lines = &script.lines;
    let cases: Vec<(Vec<String>, &str)> = vec![
        // 中间一行坏了。
        {
            let mut bad = lines.clone();
            bad[lines.len() / 2] = r#"{"seq":"#.to_string();
            (bad, "")
        },
        // 最后一行坏了（有换行，不是半行）。
        {
            let mut bad = lines.clone();
            let last = bad.len() - 1;
            bad[last] = "not json".to_string();
            (bad, "")
        },
        // 序号断了。
        {
            let mut bad = lines.clone();
            bad.remove(lines.len() / 3);
            (bad, "")
        },
        // 人话的正文不合写法。
        {
            let mut bad = lines.clone();
            let k = bad
                .iter()
                .position(|line| line.contains(r#""kind":"message.user""#))
                .expect("有人话");
            let seq = k + 1;
            bad[k] = format!(
                r#"{{"seq":{seq},"at":"2026-11-03T00:00:00.000Z","kind":"message.user","by":{{"kind":"person"}},"body":{{"blocks":7}}}}"#
            );
            (bad, "")
        },
        // 只有半行：同整份读，是空的。
        (Vec::new(), r#"{"seq":1,"kind":"sess"#),
    ];
    for (n, (bad, half)) in cases.into_iter().enumerate() {
        let (dir, _guard) = session_dir(100 + n as u64);
        write(&dir, &bad, 1, &[bad.len() / 4], half);
        let new = list(&dir, Tuning::SHIPPED).map_err(|error| format!("{error:?}"));
        let old = oracle(&dir).map_err(|error| format!("{error:?}"));
        assert_eq!(new.is_err(), n < 4, "第 {n} 种：{new:?}");
        assert_eq!(new, old, "第 {n} 种");
    }
    let (dir, _guard) = session_dir(200);
    fs::remove_dir_all(&dir).expect("删得了");
    assert!(matches!(
        list(&dir, Tuning::SHIPPED),
        Err(OpenError::Missing(_))
    ));
}

/// 读的时候还在往后写：整份读的退路只读到头一遍读到的那一条，后写的不要（同判官）。
#[test]
fn the_whole_read_stops_where_the_first_pass_stopped() {
    let script = Script::run(&mut Dice(777), 300, None);
    let (dir, _guard) = session_dir(300);
    write(&dir, &script.lines, 1, &[], "");
    let all = read_events(&dir).expect("读得出");
    let (cut_dir, _cut_guard) = session_dir(301);
    for upto in [1, all.len() / 3, all.len() / 2, all.len() - 1] {
        write(&cut_dir, &script.lines[..upto], 1, &[], "");
        let upto = Seq::new(upto as u64).expect("不是 0");
        assert_eq!(
            whole(&dir, upto).ok(),
            oracle(&cut_dir).ok(),
            "读到第 {upto} 条"
        );
    }
}

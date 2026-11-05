//! 接不上时读一遍只留要的（施工 V-43）和以前整份读、整份拼的（照抄在 `swept/old.rs`）一样：交给模型的抽取请求（连同抽到哪、
//! 这一段的几轮、说话的人怎么叫、她被叫过的名字、每一轮的群名、第一句的时刻）逐字节一样，顺手认出的末尾那一处一样，照它记进
//! 记忆日志的一样，报的错一样。
//!
//! 照种子造的会话（`swept/script.rs`：撤销、恢复、重做、撤回、压缩、撤销跨压缩、分叉、群会话里群友说话、称呼和群名换来换去、
//! 子代理、子会话、`seen` 乱写的）写成几段真文件，最后一段偶尔多半行；上次抽到哪挑在她的回复上、日志头尾、日志后面；核心刚起
//! （还接不上）和一直整份读的（日志里有撤销的）两种抽取状态；等闲的和不等闲的；放得勤的（逼出放了又撤到、再读一遍的）和出厂的。
//!
//! 和以前不一样的只有一样，单独的测试里写明：上次抽到以前的、不要的种类的正文合 JSON、不合那一种的写法的不报坏了（只解要的
//! 种类，同判官的，2026-11-03 项目主人拍板接受）。
//!
//! 默认 [`SEEDS`] 个种子；`MIYU_V43_SEEDS` 改个数（施工时跑过 5000 个）。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::assemble::Assembler;
use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::extract::candidates;
use miyu_store::log::{Mark, read_events};
use miyu_store::sweep::Tuning;
use miyu_tool::TEXT_CHARS;

use super::super::{Planning, planned};
use super::{Scratch, assembler, scratch};
use crate::memory::{
    ExtractTexts, Extractor, Landings, Plan, Prefix, Read, ReadFrom, place_people,
    read_extract_with,
};
use crate::testkit::undo_log::{Dice, UndoLog};

mod old;
mod script;

/// 平时几个种子：原来的 240 个 Windows 的 debug 要一分多钟，挪到 CI 的 randomized 那一项照 `MIYU_V43_SEEDS` 跑（施工 CI-2）。
const SEEDS: u64 = 24;

fn seeds() -> u64 {
    std::env::var("MIYU_V43_SEEDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(SEEDS)
}

/// 整份折出来的认出来的几样，照 `Debug` 写成字（同新读法顺手认出的比）。
fn prefix_of(events: &[Event]) -> String {
    format!("{:?}", Prefix::of(events))
}

/// 读出来、拼出来的：定怎么抽，和顺手认出的末尾那一处。
#[derive(Debug, PartialEq)]
struct Outcome {
    plan: Plan,
    seed: Option<(Mark, String)>,
}

/// 新读法走的哪一条。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    Swept,
    Whole,
    Mark,
}

/// 两份组装器、字、这条线自己。
struct Rig {
    scratch: Scratch,
    group: Arc<dyn Assembler + Send + Sync>,
    private: Arc<dyn Assembler + Send + Sync>,
    texts: ExtractTexts,
    own: SessionId,
}

impl Rig {
    fn new(name: &str) -> Rig {
        let scratch = scratch(name);
        Rig {
            group: assembler(&scratch, true),
            private: assembler(&scratch, false),
            texts: ExtractTexts::load(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
            )
            .expect("读得到抽取的字"),
            own: SessionId::parse(script::MAIN).expect("合写法"),
            scratch,
        }
    }

    /// 照种子挑一份拼法：群会话的有群聊近况的预算。
    fn planning(&self, group: bool, dice: &mut Dice) -> Planning<'_> {
        let assembler = match group ^ dice.odds(12) {
            true => self.group.as_ref(),
            false => self.private.as_ref(),
        };
        let budget = group.then(|| [80, 160, 540, 100_000][dice.below(4) as usize]);
        Planning {
            assembler,
            own: &self.own,
            budget,
            limit: (group && dice.odds(2)).then(|| 1 + dice.below(4) as usize),
            min_turns: 1 + dice.below(3) as usize,
            offset: UtcOffset::UTC,
            texts: &self.texts,
            name: dice.odds(2).then_some("Shorin"),
        }
    }
}

/// 以前的：整份读、整份拼。
fn was(
    dir: &Path,
    after: Seq,
    seed: bool,
    ended: bool,
    how: &Planning<'_>,
) -> Result<Outcome, String> {
    let read = old::read(dir, seed, ended)?;
    Ok(Outcome {
        plan: old::planned(read.events, after, how),
        seed: read.seed,
    })
}

/// 现在的：照抽取状态的 `landings` 读、拼。
fn now(
    dir: &Path,
    after: Seq,
    (landings, ended): (&Landings, bool),
    tuning: Tuning,
    how: &Planning<'_>,
) -> Result<(Outcome, Way), String> {
    let mut read = read_extract_with(dir, after, landings, ended, tuning)?;
    let (way, landed) = match &mut read.from {
        ReadFrom::Swept(swept) => (Way::Swept, swept.seed.take()),
        ReadFrom::Start(landed) => (Way::Whole, landed.take()),
        ReadFrom::Mark(_) => (Way::Mark, None),
    };
    let seed = landed.map(|landed| (landed.mark, format!("{:?}", landed.prefix)));
    Ok((
        Outcome {
            plan: planned(read, how),
            seed,
        },
        way,
    ))
}

/// 抽取状态：核心刚起（还接不上，`seed`）的，或者日志里有过撤销、一直整份读的。
fn landings(seed: bool) -> Landings {
    let extractor = Extractor::default();
    if !seed {
        let undo = Event::from_line(
            r#"{"seq":1,"at":"2026-11-03T00:00:00.000Z","kind":"turn.reverted","by":{"kind":"person"},"body":{"turns":[1]}}"#,
        )
        .expect("合写法");
        extractor.note_prefix(&[undo], true);
    }
    extractor.landings()
}

/// 照种子挑一种放法：多半放得勤（逼出放了又撤到、再读一遍），偶尔出厂的。
fn tuning(dice: &mut Dice) -> Tuning {
    match dice.odds(5) {
        true => Tuning::SHIPPED,
        false => Tuning {
            flush_at: 2 + dice.below(60) as usize,
            keep: dice.below(12) as usize,
        },
    }
}

/// 上次抽到哪：她的回复上、日志头尾、日志里随便一条、日志后面。
fn after(script: &script::Script, dice: &mut Dice) -> Seq {
    let last = script.first + script.lines.len() as u64 - 1;
    let n = match dice.below(8) {
        0 => 1,
        1 => last,
        2 => last + 1 + dice.below(3),
        3 => script.first + dice.below(last - script.first + 1),
        _ if script.replies.is_empty() => last,
        _ => script.replies[dice.below(script.replies.len() as u64) as usize],
    };
    Seq::new(n).expect("从 1 起")
}

/// 照 `plan` 记进记忆日志（同 `Job`：交回的候选每一轮一条、再加几个认识的人，照这一段的说话的人认，记下、记抽到哪），交回
/// 记忆日志的每一行。等的不记。
fn remembered(plan: &Plan, scratch: &Path) -> Vec<String> {
    use miyu_store::env::{Env, Platform};
    use miyu_store::recall::Room;
    use miyu_store::root::DataRoot;

    use crate::memory::{Keeper, Listener, Memory};

    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("建得了骨架");
    let memory = Memory::new(&root, None);
    let keeper = Keeper::new(
        &memory,
        Room::persona("engineer"),
        Listener::Owner(miyu_kernel::origin::By::Person(
            miyu_kernel::origin::Person::default(),
        )),
    );
    let at = Timestamp::parse("2026-11-04T00:00:00.000Z").expect("合写法");
    let session = SessionId::parse(script::MAIN).expect("合写法");
    let recorded = match plan {
        Plan::Wait => return Vec::new(),
        Plan::Skip { upto } => keeper.record_extraction(
            at,
            &session,
            *upto,
            (Vec::new(), &Default::default()),
            Some(miyu_recall::Skipped::Remembered),
        ),
        Plan::Ask {
            upto,
            turns,
            names,
            me,
            places,
            ..
        } => {
            let mut memories: Vec<serde_json::Value> = turns
                .iter()
                .map(|turn| serde_json::json!({"class": "user", "text": format!("第{turn}轮的事"), "turn": turn}))
                .collect();
            for name in names.keys().chain(me.iter()).take(4) {
                let turn = turns.iter().next().copied().unwrap_or(1);
                memories.push(serde_json::json!({"class": "person", "text": "认识的人", "who": name, "turn": turn}));
            }
            let answer = serde_json::json!({ "memories": memories }).to_string();
            let mut found = candidates(&answer, turns, TEXT_CHARS).expect("读得懂");
            let mut her = keeper.her();
            her.extend(me.iter().cloned());
            place_people(&mut found, names, &her);
            keeper.record_extraction(at, &session, *upto, (found, places), None)
        }
    };
    recorded.expect("记得进");
    read_events(&root.memory_log("engineer"))
        .expect("读得了记忆日志")
        .iter()
        .map(Event::to_line)
        .collect()
}

#[test]
fn reading_once_plans_the_same_as_reading_and_replaying_everything() {
    let rig = Rig::new("v43-same");
    let root = rig.scratch.0.join("logs");
    let (mut cases, mut swept, mut again, mut asked) = (0, 0, 0, 0);
    for seed in 0..seeds() {
        let mut dice = Dice(seed * 2_654_435_761 + 43);
        let script = script::script(&mut dice);
        let dir = root.join(format!("s{seed}"));
        UndoLog::write(&dir, script.first, &script.lines, &mut dice);
        if dice.odds(6) {
            half_line(&dir);
        }
        for _ in 0..4 {
            let after = after(&script, &mut dice);
            let (ended, seed_flag) = (dice.odds(4), !dice.odds(4));
            let tuning = tuning(&mut dice);
            let how = rig.planning(script.group, &mut dice);
            let old = was(&dir, after, seed_flag, ended, &how);
            let new = now(&dir, after, (&landings(seed_flag), ended), tuning, &how);
            let way = new.as_ref().map(|(_, way)| *way).ok();
            let new = new.map(|(outcome, _)| outcome);
            assert_eq!(
                new, old,
                "种子 {seed}：上次抽到第 {after} 条，不等闲 {ended}，接上 {seed_flag}，{tuning:?}"
            );
            cases += 1;
            swept += usize::from(way == Some(Way::Swept));
            again += usize::from(way == Some(Way::Whole));
            if let Ok(outcome) = &old
                && matches!(outcome.plan, Plan::Ask { .. })
            {
                asked += 1;
                if asked % 40 == 1 {
                    let mine = remembered(&outcome.plan, &rig.scratch.0.join(format!("m{seed}a")));
                    let theirs = remembered(
                        &new.as_ref().expect("同上").plan,
                        &rig.scratch.0.join(format!("m{seed}b")),
                    );
                    assert!(!mine.is_empty(), "记下了");
                    assert_eq!(mine, theirs, "种子 {seed}：记进记忆日志的一样");
                }
            }
        }
        miyu_store::testkit::remove_tree(&dir);
    }
    eprintln!("V-43 对照：{cases} 次，读一遍的 {swept}，退回整份读的 {again}，抽到东西的 {asked}");
    assert!(swept * 10 >= cases * 7, "多半读一遍：{swept}/{cases}");
    assert!(again > 0, "退回整份读的也比过");
    assert!(asked * 4 >= cases, "多半抽到了东西：{asked}/{cases}");
}

/// 最后一段末尾多半行（会话正写着）：新旧都跳过。
fn half_line(dir: &Path) {
    let mut segments: Vec<_> = std::fs::read_dir(dir)
        .expect("读得了目录")
        .map(|entry| entry.expect("读得了").path())
        .collect();
    segments.sort();
    let last = segments.last().expect("有段");
    let mut text = std::fs::read_to_string(last).expect("读得了");
    text.push_str(r#"{"seq":999999,"at":"2026-11"#);
    std::fs::write(last, text).expect("写得进");
}

/// 坏了的日志：一行读不成（写到一半）、少一行、要的种类的正文不合写法、不是最后一段的末尾有半行，报的错和以前一字不差。不要的
/// 种类（没派任务的工具结果）的正文合 JSON、不合那一种的写法的不报（只解要的种类，2026-11-03 项目主人拍板接受），拼出来的和
/// 那一行没坏时以前拼的一样（这一行本来就不解），认不准、退回整份读的照旧报。
#[test]
fn broken_logs_are_reported_as_before() {
    let rig = Rig::new("v43-broken");
    let root = rig.scratch.0.join("logs");
    let (mut same, mut quiet) = (0, 0);
    for seed in 0..seeds() / 2 {
        let mut dice = Dice(seed * 2_654_435_761 + 4343);
        let script = script::script(&mut dice);
        let mut lines = script.lines.clone();
        let k = 1 + dice.below(lines.len() as u64 - 1) as usize;
        let kind = dice.below(4);
        let mut unwanted = None;
        match kind {
            0 => {
                let cut = lines[k].len() / 2;
                let cut = (0..=cut)
                    .rev()
                    .find(|&at| lines[k].is_char_boundary(at))
                    .unwrap_or(0);
                lines[k].truncate(cut);
            }
            1 => {
                lines.remove(k);
            }
            2 => {
                let Some(k) = lines
                    .iter()
                    .position(|line| line.contains(r#""kind":"message.user""#))
                else {
                    continue;
                };
                lines[k] = broken_body(&lines[k], serde_json::json!({"blocks": 5}));
            }
            _ => {
                let Some(k) = lines.iter().position(|line| {
                    line.contains(r#""kind":"tool.result""#) && !line.contains("job.started")
                }) else {
                    continue;
                };
                lines[k] = broken_body(&lines[k], serde_json::json!({"call_id": 5}));
                unwanted = Some(script.first + k as u64);
            }
        }
        let dir = root.join(format!("b{seed}"));
        let fine = root.join(format!("f{seed}"));
        let mut copy = dice.clone();
        UndoLog::write(&dir, script.first, &lines, &mut dice);
        UndoLog::write(&fine, script.first, &script.lines, &mut copy);
        if kind == 1 && dice.odds(2) {
            first_segment_half_line(&dir);
        }
        for _ in 0..3 {
            let after = after(&script, &mut dice);
            let (ended, seed_flag) = (dice.odds(4), !dice.odds(4));
            let tuning = tuning(&mut dice);
            let how = rig.planning(script.group, &mut dice);
            let old = was(&dir, after, seed_flag, ended, &how);
            let new = now(&dir, after, (&landings(seed_flag), ended), tuning, &how)
                .map(|(outcome, _)| outcome);
            match unwanted {
                Some(line) if new.is_ok() => {
                    assert!(old.is_err(), "种子 {seed}：以前报坏了");
                    // 那一行写得短了，末尾那一处照字节数不一样，别的一样。
                    let fine = was(&fine, after, seed_flag, ended, &how).expect("没坏的读得了");
                    let new = new.expect("同上");
                    assert_eq!(
                        new.plan, fine.plan,
                        "种子 {seed}：第 {line} 条不解，拼出来的同那一行没坏时（上次抽到第 {after} 条）"
                    );
                    let unplaced = |seed: Option<(Mark, String)>| {
                        seed.map(|(mark, prefix)| (mark.segment, mark.next, prefix))
                    };
                    assert_eq!(unplaced(new.seed), unplaced(fine.seed), "种子 {seed}");
                    quiet += 1;
                }
                _ => {
                    assert!(
                        old.is_err() || kind == 3 || kind == 1,
                        "种子 {seed}：坏了的报"
                    );
                    assert_eq!(
                        new, old,
                        "种子 {seed}：报的错一样（第 {after} 条，{tuning:?}）"
                    );
                    same += usize::from(old.is_err());
                }
            }
        }
        miyu_store::testkit::remove_tree(&dir);
        miyu_store::testkit::remove_tree(&fine);
    }
    eprintln!("V-43 坏了的：报的一样 {same} 次，不要的种类不报 {quiet} 次");
    assert!(same > 0 && quiet > 0, "两种都比过");
}

/// 把这一行的正文换成 `body`。
fn broken_body(line: &str, body: serde_json::Value) -> String {
    let mut event: serde_json::Value = serde_json::from_str(line).expect("合 JSON");
    event["body"] = body;
    event.to_string()
}

/// 第一段末尾多半行：不是最后一段的报坏了（只有一段的是最后一段，跳过）。
fn first_segment_half_line(dir: &Path) {
    let mut segments: Vec<_> = std::fs::read_dir(dir)
        .expect("读得了目录")
        .map(|entry| entry.expect("读得了").path())
        .collect();
    segments.sort();
    let mut text = std::fs::read_to_string(&segments[0]).expect("读得了");
    text.push_str(r#"{"seq":"#);
    std::fs::write(&segments[0], text).expect("写得进");
}

/// 从头读的（整份读、读一遍只留要的，施工 V-43）顺手认出了末尾那一处的，交给抽取状态接上；交回有没有。
pub(super) fn seed_from(extractor: &Extractor, read: &mut Read) -> bool {
    let landed = match &mut read.from {
        ReadFrom::Start(landed) => landed.take(),
        ReadFrom::Swept(swept) => swept.seed.take(),
        ReadFrom::Mark(_) => None,
    };
    landed.map(|landed| extractor.seed(landed)).is_some()
}

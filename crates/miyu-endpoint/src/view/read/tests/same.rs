//! 新旧逐字比（施工 V-41 上）：照种子造会话日志（`script.rs`），写成几段真文件（一段、几段，中间夹空段、最后一段是空的、
//! 末尾半行），以前的读法（`old.rs`：整份读、整份切、整份学）和现在的（只读要的那一截）各算一遍，每一页、每种参数的回应
//! 逐字一样：从最新一页翻到头（几种轮数、几种字节上限，交事件的、交条目的），日志里每一处当 `before`；视图流订阅的最新一页
//! （到日志里的各处为止），连同投影的会话状态，再把后面的事件接着喂两台投影，每一条交出的变化、会话状态都一样。
//!
//! 坏行（读不成的、序号写错的）：以前的整份读每一页都报坏了；现在的读到了那一行的报坏了（那一行在这一页里的一定报），没读到
//! 的照没坏算，交的和没坏的那份一样。

use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use miyu_kernel::event::Event;
use miyu_store::blob::Blobs;
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_view::{Projector, Texts};

use super::old;
use super::script::{Dice, Script};
use crate::test_support::{TempDir, resources, temp_root};
use crate::view::{CAP, TURNS, newest_of, paged, project, read::read, size};

/// 投影要的字：英文的那一份，几千份会话共用。
fn texts() -> Arc<Texts> {
    Arc::new(project::texts(&ResourceRoot::at(resources()), "en").expect("出厂的字读得成"))
}

/// 照种子 `seed` 造一份会话日志，写进 `dir`。交回造好的行（照序号，不含末尾的半行）。
fn made(seed: u64, dir: &Path) -> Vec<String> {
    let mut dice = Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let fork = if dice.odds(8) {
        30 + dice.below(400)
    } else {
        1
    };
    let mut script = Script::new(fork, dice.odds(6));
    let steps = match dice.below(10) {
        0 => dice.below(4),
        1..=6 => 10 + dice.below(80),
        _ => 100 + dice.below(400),
    };
    script.play(&mut dice, steps);
    let lines = script.lines;
    write(&mut dice, dir, script.first, &lines);
    lines
}

/// 照种子切成几段写进 `dir`：一段到四段，有时中间夹一段空的、最后一段是空的，有时末尾一行没写完。
fn write(dice: &mut Dice, dir: &Path, first: u64, lines: &[String]) {
    fs::create_dir_all(dir).unwrap();
    let mut cuts: Vec<usize> = (0..dice.below(4))
        .map(|_| usize::try_from(dice.below(lines.len() as u64)).unwrap_or(0))
        .filter(|&cut| cut > 0)
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut segments: Vec<(u64, String)> = Vec::new();
    let mut from = 0;
    for cut in cuts.into_iter().chain([lines.len()]) {
        let text: String = lines[from..cut]
            .iter()
            .map(|line| format!("{line}\n"))
            .collect();
        let name = first + from as u64;
        // 前一段至少两行的，中间夹一段空的：名字排在两段之间。
        if from > 1 && dice.odds(5) && segments.last().is_some_and(|(prev, _)| *prev < name - 1) {
            segments.push((name - 1, String::new()));
        }
        segments.push((name, text));
        from = cut;
    }
    if dice.odds(6) {
        segments.push((first + lines.len() as u64, String::new()));
    }
    if dice.odds(4)
        && let Some((_, last)) = segments.last_mut()
    {
        last.push_str(r#"{"seq":"#);
    }
    for (name, text) in segments {
        fs::write(dir.join(format!("{name:012}.jsonl")), text).unwrap();
    }
}

/// 以前的、现在的各算一页，比；交回这一页的 `first`、`more`。
fn same_page(
    (dir, events): (&Path, &[Event]),
    before: Option<u64>,
    (turns, cap): (usize, usize),
    view: Option<(&Arc<Texts>, &Blobs)>,
    what: &str,
) -> (Option<u64>, bool) {
    let view = view.map(|(texts, blobs)| (Arc::clone(texts), blobs.clone()));
    let (old, _) = old::paged(events, before, (turns, cap), view.clone());
    let slice = read(dir, before, (turns, cap), view.is_some(), &size)
        .unwrap_or_else(|error| panic!("{what} before {before:?}: {error}"));
    let (new, _) = paged(&slice, view);
    assert_eq!(new, old, "{what} before {before:?} turns {turns} cap {cap}");
    (old["first"].as_u64(), old["more"] == Value::Bool(true))
}

/// 视图流订阅时的最新一页（到第 `upto` 条为止）比回应、会话状态，再把后面的接着喂两台投影，每一条比。
fn same_stream(
    (dir, events): (&Path, &[Event]),
    upto: u64,
    cap: usize,
    (texts, blobs): (&Arc<Texts>, &Blobs),
    what: &str,
) {
    let view = Some((Arc::clone(texts), blobs.clone()));
    let until = events.partition_point(|event| event.seq.get() <= upto);
    let (old, old_projector) = old::paged(&events[..until], None, (TURNS, cap), view.clone());
    let slice = if cap == CAP {
        newest_of(dir, upto)
    } else {
        read(dir, upto.checked_add(1), (TURNS, cap), true, &size)
    }
    .unwrap_or_else(|error| panic!("{what} upto {upto}: {error}"));
    let (new, new_projector) = paged(&slice, view);
    assert_eq!(new, old, "{what} upto {upto} cap {cap}");
    let (mut old_projector, mut new_projector): (Projector, Projector) = (
        old_projector.expect("交了字"),
        new_projector.expect("交了字"),
    );
    assert_eq!(
        new_projector.status(),
        old_projector.status(),
        "{what} upto {upto}"
    );
    for event in &events[until..] {
        let (old_changes, new_changes) = (old_projector.event(event), new_projector.event(event));
        assert_eq!(
            new_changes, old_changes,
            "{what} upto {upto} at {}",
            event.seq
        );
        assert_eq!(
            new_projector.status(),
            old_projector.status(),
            "{what} upto {upto} at {}",
            event.seq
        );
    }
    assert_eq!(
        new_projector.entries(),
        old_projector.entries(),
        "{what} upto {upto}"
    );
}

/// 一份会话从头到尾比：翻到头、每一处当 `before`、订阅到各处。
fn same_session(seed: u64, place: &TempDir, (texts, blobs): (&Arc<Texts>, &Blobs)) {
    let dir = place.join(format!("s{seed}"));
    made(seed, &dir);
    let events = read_events(&dir).expect("造的日志读得成");
    let what = format!("seed {seed}");
    let mut dice = Dice(seed | 1);
    let log = (dir.as_path(), events.as_slice());
    let first = events.first().map_or(1, |event| event.seq.get());
    let end = events.last().map_or(first, |event| event.seq.get() + 1);
    // 翻到头：几种轮数、字节上限，交事件的、交条目的。
    for (turns, cap, view) in [
        (TURNS, CAP, true),
        (1 + usize::try_from(dice.below(3)).unwrap_or(0), CAP, false),
        (
            *[1, 2, 3, 5, 50]
                .get(usize::try_from(dice.below(5)).unwrap_or(0))
                .unwrap_or(&1),
            600 + usize::try_from(dice.below(4000)).unwrap_or(0),
            dice.odds(2),
        ),
    ] {
        let view = view.then_some((texts, blobs));
        let mut before = None;
        let mut pages = 0;
        loop {
            let (first, more) = same_page(log, before, (turns, cap), view, &what);
            pages += 1;
            assert!(pages <= events.len() + 1, "{what} 翻不到头");
            match (first, more) {
                (Some(first), true) => before = Some(first),
                _ => break,
            }
        }
    }
    // 每一处当 `before`（多的照种子挑），日志前面、后面的也算。
    let all: Vec<u64> = (1..=end + 2).collect();
    let picked: Vec<u64> = if all.len() <= 120 {
        all
    } else {
        (0..60).map(|_| 1 + dice.below(end + 2)).collect()
    };
    for before in picked {
        let turns = *[1, 2, 3, 20, 50]
            .get(usize::try_from(dice.below(5)).unwrap_or(0))
            .unwrap_or(&20);
        let cap = if dice.odds(3) {
            800 + usize::try_from(dice.below(3000)).unwrap_or(0)
        } else {
            CAP
        };
        let view = dice.odds(2).then_some((texts, blobs));
        same_page(log, Some(before), (turns, cap), view, &what);
    }
    // 视图流订阅：到最后一条、到中间几处为止。
    let mut uptos = vec![end - 1];
    uptos.extend((0..3).map(|_| first + dice.below(end - first)));
    for upto in uptos {
        let cap = if dice.odds(3) {
            800 + usize::try_from(dice.below(3000)).unwrap_or(0)
        } else {
            CAP
        };
        same_stream(log, upto, cap, (texts, blobs), &what);
    }
    miyu_store::testkit::remove_tree(&dir);
}

/// 第 `part` 份种子：每份五百个，平时只跑前十五个（Windows CI 的门禁一趟限 45 分钟，六十个会超），`MIYU_PAGES_FULL=1` 的全跑
/// （施工时 debug、release 都跑过一遍）。
fn seeds(part: u64) {
    let each = if std::env::var_os("MIYU_PAGES_FULL").is_some() {
        500
    } else {
        15
    };
    let (root, place) = temp_root("v41-same");
    let blobs = Blobs::new(root.blobs());
    let texts = texts();
    for seed in part * 500..part * 500 + each {
        same_session(seed, &place, (&texts, &blobs));
    }
}

// 三千份，分六个测试并着跑。
#[test]
fn every_page_reads_like_the_whole_0() {
    seeds(0);
}

#[test]
fn every_page_reads_like_the_whole_1() {
    seeds(1);
}

#[test]
fn every_page_reads_like_the_whole_2() {
    seeds(2);
}

#[test]
fn every_page_reads_like_the_whole_3() {
    seeds(3);
}

#[test]
fn every_page_reads_like_the_whole_4() {
    seeds(4);
}

#[test]
fn every_page_reads_like_the_whole_5() {
    seeds(5);
}

/// 一行坏了：读不成（截掉后半行）或者序号写错了。
fn spoil(seed: u64, lines: &[String], dir: &Path) -> Option<u64> {
    let mut dice = Dice(seed ^ 0x5eed | 1);
    let k = usize::try_from(dice.below(lines.len() as u64)).ok()?;
    let line = &lines[k];
    let event = Event::from_line(line).ok()?;
    let seq = event.seq.get();
    let spoiled = if dice.odds(2) {
        line[..line.len() / 2].to_string()
    } else if line.starts_with(&format!(r#"{{"seq":{seq},"#)) {
        line.replacen(
            &format!(r#""seq":{seq}"#),
            &format!(r#""seq":{}"#, seq + 1),
            1,
        )
    } else {
        line[..line.len() - 1].to_string()
    };
    // 照原来的段写回：找到这一行在哪一段。
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let wanted = format!("{line}\n");
        if let Some(at) = text.find(&wanted) {
            let text = format!("{}{spoiled}\n{}", &text[..at], &text[at + wanted.len()..]);
            fs::write(&path, text).unwrap();
            return Some(seq);
        }
    }
    None
}

#[test]
fn a_broken_line_is_reported_when_read_and_counted_past_when_not() {
    let (root, place) = temp_root("v41-broken");
    let blobs = Blobs::new(root.blobs());
    let texts = texts();
    let (mut reported, mut past) = (0, 0);
    for seed in 0..160 {
        let clean = place.join(format!("c{seed}"));
        let lines = made(seed, &clean);
        let broken = place.join(format!("b{seed}"));
        made(seed, &broken);
        let Some(seq) = spoil(seed, &lines, &broken) else {
            continue;
        };
        assert!(
            read_events(&broken).is_err(),
            "seed {seed}: 以前的整份读报坏了"
        );
        let mut dice = Dice(seed | 1);
        let mut before = None;
        for _ in 0..40 {
            let turns = *[1, 2, 20]
                .get(usize::try_from(dice.below(3)).unwrap_or(0))
                .unwrap_or(&20);
            let view = dice.odds(2).then(|| (Arc::clone(&texts), blobs.clone()));
            let want =
                read(&clean, before, (turns, CAP), view.is_some(), &size).expect("没坏的读得成");
            let (want, _) = paged(&want, view.clone());
            match read(&broken, before, (turns, CAP), view.is_some(), &size) {
                Ok(slice) => {
                    let (got, _) = paged(&slice, view);
                    assert_eq!(got, want, "seed {seed} 坏在 {seq}，before {before:?}");
                    let page = if got["entries"].is_null() {
                        &got["events"]
                    } else {
                        &Value::Null
                    };
                    if let Some(page) = page.as_array() {
                        assert!(
                            page.iter().all(|event| event["seq"].as_u64() != Some(seq)),
                            "seed {seed}: 坏的那一行在这一页里，一定读到、报坏了"
                        );
                    }
                    past += 1;
                }
                Err(_) => reported += 1,
            }
            match (want["first"].as_u64(), want["more"] == Value::Bool(true)) {
                (Some(first), true) => before = Some(first),
                _ => break,
            }
        }
        miyu_store::testkit::remove_tree(&clean);
        miyu_store::testkit::remove_tree(&broken);
    }
    assert!(
        reported > 30 && past > 30,
        "两样都有：报了 {reported}、没读到 {past}"
    );
}

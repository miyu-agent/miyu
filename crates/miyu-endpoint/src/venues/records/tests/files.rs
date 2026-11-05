//! 照真的日志文件读（施工 V-39 上）：分几段的、末尾半行的、中间坏一行的、序号断了的、段名不对的、没有这个会话的，新旧的回应
//! 和报错（哪一种、先后）一样。坏了的照旧报坏了，不管坏在要判的那一条前面还是后面；读不出钉下的字的报内部错，排在参数错前面、
//! 坏了后面。
//!
//! 施工 V-39 上补：新的（会话目录只解判官要的种类）另和 V-39 上的（每一行全解，`old::Full`）比；不要的种类的正文合 JSON、
//! 不合写法的，新的不报坏了（项目主人拍板接受），外壳坏了、不是合法 JSON 的照报。

use std::fs;
use std::path::Path;

use miyu_assemble::GroupChat;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;
use miyu_store::log::read_events;

use super::old;
use super::rendering;
use super::same::{WINDOWS, events};
use crate::refusal::Refusal;
use crate::sweep::Tuning;
use crate::test_support::temp_root;
use crate::venues::records::read::{Dir, judge};

/// 把 `events` 写成几段：`cuts` 是各段从第几条（下标）起。
fn write(dir: &Path, events: &[Event], cuts: &[usize]) -> Vec<String> {
    fs::create_dir_all(dir).expect("建得了");
    let lines: Vec<String> = events.iter().map(Event::to_line).collect();
    let mut starts = vec![0];
    starts.extend(
        cuts.iter()
            .copied()
            .filter(|&cut| cut > 0 && cut < lines.len()),
    );
    starts.dedup();
    for (k, &start) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(lines.len());
        let mut text = String::new();
        for line in &lines[start..end] {
            text.push_str(line);
            text.push('\n');
        }
        fs::write(dir.join(format!("{:012}.jsonl", start + 1)), text).expect("写得了");
    }
    lines
}

/// 钉下的字照第一条找：它要全解（`session.created`）。
fn texts(
    first: Option<&Event>,
) -> Result<(GroupChat, Option<miyu_assemble::ForwardTexts>), Refusal> {
    assert!(
        matches!(
            first.map(|event| &event.body),
            Some(Body::SessionCreated(_))
        ),
        "第一条要全解：{first:?}"
    );
    Ok((rendering(), None))
}

/// 新旧各读一遍 `dir`，比。
fn same(dir: &Path, msg: Seq, k: usize, tuning: Tuning) -> Result<String, Refusal> {
    let window = WINDOWS[k % WINDOWS.len()];
    let new = judge(&mut Dir(dir.to_path_buf()), msg, window, texts, tuning);
    let old = old::judge(read_events(dir), msg, window, texts);
    assert_eq!(new, old, "{} 第 {msg} 条", dir.display());
    let full = judge(
        &mut old::Full(dir.to_path_buf()),
        msg,
        window,
        texts,
        tuning,
    );
    assert_eq!(new, full, "{} 第 {msg} 条（V-39 上）", dir.display());
    new.map(|records| records.records + &records.current)
}

/// 平时造几份：全量 150 份，Windows 的 debug 要八十秒上下（施工 CI-2）；`MIYU_RECORDS_FULL` 设了的全量。
const FILES_QUICK: u64 = 15;

/// 全量造几份。
const FILES_FULL: u64 = 150;

#[test]
fn logs_in_segments_read_the_same_as_before() {
    let (_root, temp) = temp_root("v39-files");
    let seeds = match std::env::var_os("MIYU_RECORDS_FULL") {
        Some(_) => FILES_FULL,
        None => FILES_QUICK,
    };
    let mut judged = 0;
    for seed in 1..=seeds {
        let events = events(seed);
        let dir = temp.join(format!("s{seed}"));
        let n = events.len();
        write(
            &dir,
            &events,
            &[n / 3, n / 3 + 1 + (seed as usize % 5), n * 3 / 4],
        );
        let tuning = Tuning {
            flush_at: 1 + seed as usize % 9,
            keep: 1 + seed as usize % 4,
        };
        // 每四条判一条（真文件读得慢），轮着换窗口；最后一条都判。
        for (k, event) in events.iter().enumerate() {
            if (k + seed as usize).is_multiple_of(4) || k + 1 == n {
                judged += usize::from(same(&dir, event.seq, k / 4, tuning).is_ok());
            }
        }
    }
    // 全量一千条以上；平时照份数折。
    let least = 1000 * seeds / FILES_FULL;
    assert!(judged as u64 > least, "判成了的太少：{judged}");
}

/// 怎么弄坏：目录、原来的各行。
type Spoil<'a> = Box<dyn Fn(&Path, &[String]) + 'a>;

/// 日志坏了的各样：新旧报一样的错。
#[test]
fn a_broken_log_is_refused_the_same() {
    let (_root, temp) = temp_root("v39-broken");
    let events = events(11);
    let n = events.len();
    let msg = events
        .iter()
        .rev()
        .find(|event| matches!(event.body, miyu_kernel::event::Body::MessageUser(_)))
        .expect("有人话")
        .seq;
    let early = Seq::new(3).expect("合写法");
    let tuning = Tuning::SHIPPED;
    let cases: [(&str, Spoil<'_>, Option<Refusal>); 7] = [
        (
            "中间一行坏了",
            Box::new(|dir, lines| {
                rewrite(dir, 1, |line| {
                    Some(if line == lines[n / 2 + 2] {
                        line.replacen('{', "<", 1)
                    } else {
                        line.to_string()
                    })
                })
            }),
            Some(Refusal::BROKEN),
        ),
        (
            "末尾半行",
            Box::new(|dir, _| append(dir, None, "{\"seq\":")),
            None,
        ),
        (
            "不是最后一段的末尾半行",
            Box::new(|dir, _| append(dir, Some(0), "{\"seq\"")),
            Some(Refusal::BROKEN),
        ),
        (
            "序号断了",
            Box::new(|dir, lines| {
                rewrite(dir, 1, |line| {
                    if line == lines[n / 2 + 1] {
                        None
                    } else {
                        Some(line.to_string())
                    }
                })
            }),
            Some(Refusal::BROKEN),
        ),
        (
            "段名不对",
            Box::new(|dir, _| rename_first(dir)),
            Some(Refusal::BROKEN),
        ),
        (
            "最后一段坏在要判的那一条后面",
            Box::new(|dir, _| append(dir, None, "not json\n")),
            Some(Refusal::BROKEN),
        ),
        (
            "一段都没有",
            Box::new(|dir, _| clear(dir)),
            Some(Refusal::NOT_FOUND),
        ),
    ];
    for (k, (what, spoil, expected)) in cases.iter().enumerate() {
        let dir = temp.join(format!("b{k}"));
        let lines = write(&dir, &events, &[n / 2, n - 1]);
        spoil(&dir, &lines);
        for at in [early, msg] {
            let got = same(&dir, at, k, tuning);
            if let Some(expected) = expected {
                assert_eq!(got.as_ref().err(), Some(expected), "{what}");
            }
        }
    }
    let missing = temp.join("没有的");
    assert_eq!(same(&missing, msg, 0, tuning), Err(Refusal::NOT_FOUND));
}

/// 施工 V-39 上补：坏在不要的种类（工具结果、回复）里。正文合 JSON、不合写法的，新的不报坏了，回应同那一行没坏时（V-39 上
/// 报坏了）；外壳坏了、正文不是合法 JSON 的照报。坏在要的种类（人话）的正文里的照报。
#[test]
fn a_skipped_body_is_not_checked_but_its_shell_is() {
    let (_root, temp) = temp_root("v39a2-skipped");
    let seed = (1..=200_u64)
        .find(|&seed| {
            let events = events(seed);
            events.len() > 40
                && events[..events.len() / 2]
                    .iter()
                    .any(|event| matches!(event.body, Body::ToolResult(_)))
                && events[..events.len() / 2]
                    .iter()
                    .any(|event| matches!(event.body, Body::MessageAssistant(_)))
        })
        .expect("有带工具结果、回复的");
    let events = events(seed);
    let half = events.len() / 2;
    let first_of = |want: fn(&Body) -> bool| {
        events[..half]
            .iter()
            .position(|event| want(&event.body))
            .expect("有")
    };
    let tool = first_of(|body| matches!(body, Body::ToolResult(_)));
    let reply = first_of(|body| matches!(body, Body::MessageAssistant(_)));
    let user = first_of(|body| matches!(body, Body::MessageUser(_)));
    let msg = events
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::MessageUser(_)))
        .expect("有人话")
        .seq;
    let fine = temp.join("fine");
    write(&fine, &events, &[half]);
    let expected = old::judge(read_events(&fine), msg, WINDOWS[0], texts);
    assert!(expected.is_ok(), "{expected:?}");
    // 第几行、怎么改、新的照读（`true`）还是报坏了。
    let cases: [(usize, &str, &str, bool); 7] = [
        (tool, r#""status":"ok""#, r#""status":5"#, true),
        (reply, r#""blocks":["#, r#""blocks":7,"x":["#, true),
        (tool, r#""status":"ok""#, r#""status":ok"#, false),
        (tool, r#"{"seq":"#, r#"{"seq":-"#, false),
        (tool, r#""at":"2026"#, r#""at":"1"#, false),
        (reply, r#""seen":"#, r#""seen":"x","y":"#, false),
        (user, r#""blocks":["#, r#""blocks":7,"x":["#, false),
    ];
    for (k, &(line, from, to, read)) in cases.iter().enumerate() {
        let dir = temp.join(format!("c{k}"));
        let lines = write(&dir, &events, &[half]);
        assert!(lines[line].contains(from), "第 {k} 样：{}", lines[line]);
        let spoiled = lines[line].replacen(from, to, 1);
        rewrite(&dir, 0, |one| {
            Some(if one == lines[line] {
                spoiled.clone()
            } else {
                one.to_string()
            })
        });
        for at in [events[line].seq, msg] {
            let new = judge(
                &mut Dir(dir.clone()),
                at,
                WINDOWS[0],
                texts,
                Tuning::SHIPPED,
            );
            let full = judge(
                &mut old::Full(dir.clone()),
                at,
                WINDOWS[0],
                texts,
                Tuning::SHIPPED,
            );
            assert_eq!(full, Err(Refusal::BROKEN), "第 {k} 样 V-39 上报坏了");
            if read {
                let fine = judge(
                    &mut Dir(fine.clone()),
                    at,
                    WINDOWS[0],
                    texts,
                    Tuning::SHIPPED,
                );
                assert_eq!(new, fine, "第 {k} 样照读，同没坏的");
            } else {
                assert_eq!(new, Err(Refusal::BROKEN), "第 {k} 样照报");
            }
        }
    }
}

/// 要判的那一条不在、不是场所人话、是撤销那一条：参数错；钉下的字读不出：内部错，在参数错前面、坏了后面。
#[test]
fn refusals_keep_their_order() {
    let (_root, temp) = temp_root("v39-order");
    let events = events(23);
    let dir = temp.join("s");
    write(&dir, &events, &[]);
    let past = Seq::new(events.len() as u64 + 5).expect("合写法");
    assert_eq!(
        same(&dir, past, 0, Tuning::SHIPPED),
        Err(Refusal::BAD_PARAMS)
    );
    assert_eq!(
        same(&dir, Seq::FIRST, 1, Tuning::SHIPPED),
        Err(Refusal::BAD_PARAMS)
    );
    let unread = |_: Option<&Event>| Err(Refusal::INTERNAL);
    let new = judge(
        &mut Dir(dir.clone()),
        past,
        WINDOWS[0],
        unread,
        Tuning::SHIPPED,
    );
    assert_eq!(new, Err(Refusal::INTERNAL));
    append(&dir, None, "坏的\n");
    let new = judge(
        &mut Dir(dir.clone()),
        past,
        WINDOWS[0],
        unread,
        Tuning::SHIPPED,
    );
    assert_eq!(new, Err(Refusal::BROKEN));
}

/// 第 `k` 段（照名字排）的每一行照 `change` 改写，交回空的删掉那一行。
fn rewrite(dir: &Path, k: usize, change: impl Fn(&str) -> Option<String>) {
    let path = segments(dir)[k].clone();
    let text = fs::read_to_string(&path).expect("读得了");
    let lines: Vec<String> = text.lines().filter_map(&change).collect();
    let mut out = lines.join("\n");
    out.push('\n');
    fs::write(&path, out).expect("写得了");
}

/// 第 `k` 段（没有的是最后一段）末尾接上 `text`。
fn append(dir: &Path, k: Option<usize>, text: &str) {
    let all = segments(dir);
    let path = k.map_or_else(|| all.last().expect("有段").clone(), |k| all[k].clone());
    let mut all = fs::read_to_string(&path).expect("读得了");
    all.push_str(text);
    fs::write(&path, all).expect("写得了");
}

/// 第一段改成名字大一号的。
fn rename_first(dir: &Path) {
    let first = segments(dir)[0].clone();
    fs::rename(&first, dir.join("000000000002.jsonl")).expect("改得了名");
}

/// 段都删掉，目录留着。
fn clear(dir: &Path) {
    for path in segments(dir) {
        fs::remove_file(path).expect("删得了");
    }
}

fn segments(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut all: Vec<_> = fs::read_dir(dir)
        .expect("有目录")
        .map(|entry| entry.expect("读得了").path())
        .collect();
    all.sort();
    all
}

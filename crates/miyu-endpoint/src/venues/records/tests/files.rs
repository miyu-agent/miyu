//! 照真的日志文件读（施工 V-39 上）：分几段的、末尾半行的、中间坏一行的、序号断了的、段名不对的、没有这个会话的，新旧的回应
//! 和报错（哪一种、先后）一样。坏了的照旧报坏了，不管坏在要判的那一条前面还是后面；读不出钉下的字的报内部错，排在参数错前面、
//! 坏了后面。

use std::fs;
use std::path::Path;

use miyu_assemble::GroupChat;
use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;
use miyu_store::log::read_events;

use super::old;
use super::rendering;
use super::same::{WINDOWS, events};
use crate::refusal::Refusal;
use crate::test_support::temp_root;
use crate::venues::records::pass::Tuning;
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

fn texts(_: Option<&Event>) -> Result<(GroupChat, Option<miyu_assemble::ForwardTexts>), Refusal> {
    Ok((rendering(), None))
}

/// 新旧各读一遍 `dir`，比。
fn same(dir: &Path, msg: Seq, k: usize, tuning: Tuning) -> Result<String, Refusal> {
    let window = WINDOWS[k % WINDOWS.len()];
    let new = judge(&mut Dir(dir.to_path_buf()), msg, window, texts, tuning);
    let old = old::judge(read_events(dir), msg, window, texts);
    assert_eq!(new, old, "{} 第 {msg} 条", dir.display());
    new.map(|records| records.records + &records.current)
}

#[test]
fn logs_in_segments_read_the_same_as_before() {
    let (_root, temp) = temp_root("v39-files");
    let mut judged = 0;
    for seed in 1..=150_u64 {
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
    assert!(judged > 1000, "判成了的太少：{judged}");
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

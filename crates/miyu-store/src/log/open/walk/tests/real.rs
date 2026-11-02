//! 真数据形状的对照（施工 V-37 中）：`MIYU_V37_ROOT` 指一个数据根的副本（或者直接指放会话目录的那一层，数据根的布局换了
//! 也用得上），里面每个会话新旧逐条比 `to_line()`、比报错：从头只读（默认的窗口和 64 KiB 的窗口）、从几处记下的位置读起、
//! 截半行（在临时目录里各拷一份截，不碰 `MIYU_V37_ROOT`）。
//!
//! `MIYU_V37_ROOT=<数据根的副本> cargo test -p miyu-store --lib real_shaped -- --ignored --nocapture`

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::*;

/// 把 `from` 里的文件拷到 `to`（一个会话的目录，只有一层）。
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

/// 读一遍，交回读到的和用了多久。
fn timed(
    reader: Reader<'_>,
    dir: &Path,
    half: HalfLine,
    from: Option<&Mark>,
) -> (Outcome, Duration) {
    let at = Instant::now();
    let outcome = read(reader, dir, half, from, Stop::Never);
    (outcome, at.elapsed())
}

/// 每一段里大致等分的几处行开头。
fn some_marks(dir: &Path) -> Vec<Mark> {
    let mut all = Vec::new();
    let mut names: Vec<(u64, PathBuf)> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            let stem = path
                .file_name()?
                .to_str()?
                .strip_suffix(".jsonl")?
                .to_string();
            Some((stem.parse().ok()?, path))
        })
        .collect();
    names.sort();
    for (first, path) in names {
        let bytes = fs::read(&path).unwrap();
        let starts: Vec<usize> = std::iter::once(0)
            .chain(memchr::memchr_iter(b'\n', &bytes).map(|at| at + 1))
            .collect();
        for k in (0..starts.len()).step_by(starts.len().div_ceil(8).max(1)) {
            all.push(Mark {
                segment: first,
                bytes: starts[k] as u64,
                next: Seq::new(first + k as u64).unwrap(),
            });
        }
    }
    all
}

#[test]
#[ignore = "要 MIYU_V37_ROOT 指一个数据根的副本"]
fn real_shaped_sessions_read_the_same() {
    let root = PathBuf::from(std::env::var_os("MIYU_V37_ROOT").expect("要 MIYU_V37_ROOT"));
    let scratch = Scratch::new();
    let under = root.join("sessions");
    let under = if under.is_dir() { under } else { root.clone() };
    let mut sessions: Vec<PathBuf> = fs::read_dir(&under)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    sessions.sort();
    assert!(!sessions.is_empty(), "{} 里没有会话", root.display());
    let (mut events, mut bytes, mut marks) = (0, 0, 0);
    let (mut old_time, mut new_time) = (Duration::ZERO, Duration::ZERO);
    for dir in &sessions {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let size: u64 = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().metadata().unwrap().len())
            .sum();
        let (old, spent) = timed(&before, dir, HalfLine::Skip, None);
        old_time += spent;
        for window in [WINDOW, 65_536] {
            let now = move |dir: &Path,
                            half: HalfLine,
                            from: Option<&Mark>,
                            sink: &mut dyn FnMut(Piece) -> Flow| {
                walk_in(dir, half, from, window, sink)
            };
            let (new, spent) = timed(&now, dir, HalfLine::Skip, None);
            if window == WINDOW {
                new_time += spent;
            }
            agree(&old, &new, false, &format!("{name}：只读，窗口 {window}"));
        }
        let now = |dir: &Path,
                   half: HalfLine,
                   from: Option<&Mark>,
                   sink: &mut dyn FnMut(Piece) -> Flow| {
            walk_in(dir, half, from, WINDOW, sink)
        };
        for mark in some_marks(dir) {
            let old = read(&before, dir, HalfLine::Skip, Some(&mark), Stop::Never);
            let new = read(&now, dir, HalfLine::Skip, Some(&mark), Stop::Never);
            agree(&old, &new, false, &format!("{name}：从 {mark:?} 读起"));
            marks += 1;
        }
        let a = scratch.path().join(format!("{name}-old"));
        let b = scratch.path().join(format!("{name}-new"));
        copy(dir, &a);
        copy(dir, &b);
        let old_cut = read(&before, &a, HalfLine::Cut, None, Stop::Never);
        let new_cut = read(&now, &b, HalfLine::Cut, None, Stop::Never);
        agree(&old_cut, &new_cut, false, &format!("{name}：截半行"));
        assert!(files(&a) == files(&b), "{name}：截出来的文件不一样");
        let count = old.0.iter().filter(|one| one.is_some()).count();
        eprintln!("{name}：{count} 条、{size} 字节，交回 {}", old.1);
        events += count;
        bytes += size;
    }
    eprintln!(
        "{} 个会话、{events} 条、{bytes} 字节、{marks} 处记下的位置逐条一样；从头只读（debug）以前 {old_time:?}，现在 {new_time:?}",
        sessions.len()
    );
}

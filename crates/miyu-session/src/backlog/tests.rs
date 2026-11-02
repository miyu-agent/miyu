//! 补发照序号读（施工 V-37 下）：从第 `after + 1` 条定位着读、读到 `upto` 就停，交回的事件、写好的行和原来一段段从头读到
//! 最后一段末尾再挑的（照抄留在这里的 [`old`]）逐个一样；`after`、`upto` 落在各段各处；`upto` 在第一段中间的，后面的段
//! 不打开；`after` 以前的不解、不自检，`upto` 以后的不读（2026-11-03 项目主人定）。

use std::fs;
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;
use miyu_tool::Log;

use super::Backlog;
use crate::store::LogDir;

/// 临时目录：用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let path = std::env::temp_dir().join(format!("miyu-backlog-{name}-{}", std::process::id()));
        drop(fs::remove_dir_all(&path));
        fs::create_dir_all(&path).expect("建得了");
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// 第 `seq` 条的一行（带换行），正文长短照序号变。
fn line(seq: u64) -> String {
    let text = "句".repeat(usize::try_from(seq % 13).unwrap_or(0));
    format!(
        "{{\"seq\":{seq},\"at\":\"2026-11-03T07:00:00.000Z\",\"kind\":\"message.user\",\"by\":{{\"kind\":\"person\"}},\"body\":{{\"blocks\":[{{\"type\":\"text\",\"text\":\"第 {seq} {text}\"}}]}}}}\n"
    )
}

/// 第 `from` 到第 `to` 条。
fn lines(from: u64, to: u64) -> String {
    (from..=to).map(line).collect()
}

/// 照 `segments`（名字、内容）写好一个会话的日志。
fn write(dir: &Path, segments: &[(u64, String)]) {
    fs::create_dir_all(dir).expect("建得了");
    for (first, text) in segments {
        fs::write(dir.join(format!("{first:012}.jsonl")), text).expect("写得进");
    }
}

/// 原来的读法（施工 3-8 六补起，V-37 下以前的 `Backlog::read`），照抄：一段段从头读到最后一段的末尾，留 `after` 以后、
/// `upto` 为止的。
fn old(dir: &Path, after: u64, upto: u64) -> Result<Vec<Event>, String> {
    if after >= upto {
        return Ok(Vec::new());
    }
    let log = Log::new(LogDir(dir.to_path_buf()));
    let mut kept = Vec::new();
    log.read(|segment| {
        kept.extend(segment.into_iter().filter(|event| {
            let seq = event.seq.get();
            seq > after && seq <= upto
        }));
        true
    })?;
    Ok(kept)
}

fn backlog(dir: &Path, after: u64, upto: u64) -> Backlog {
    Backlog::new(Log::new(LogDir(dir.to_path_buf())), after, upto)
}

/// 推出去的那一行的写法：测试里随便一个，带上序号和原样的事件。
fn written(event: &Event) -> String {
    format!("{}|{}", event.seq, event.to_line())
}

/// 新的两样（事件、写好的行）和原来的比。
async fn same_as_old(dir: &Path, after: u64, upto: u64) {
    let old = old(dir, after, upto);
    let read = backlog(dir, after, upto).read().await;
    let lines = backlog(dir, after, upto).lines(written).await;
    match old {
        Ok(old) => {
            assert_eq!(read.as_ref(), Ok(&old), "事件：after {after} upto {upto}");
            let want: Vec<String> = old.iter().map(written).collect();
            assert_eq!(lines, Ok(want), "行：after {after} upto {upto}");
        }
        Err(why) => {
            assert_eq!(read, Err(why.clone()), "报错：after {after} upto {upto}");
            assert_eq!(lines, Err(why), "报错：after {after} upto {upto}");
        }
    }
}

/// 三段（第二段后面夹着一段空的，名字 55），最后一段末尾有没写完的半行：`after`、`upto` 落在每一处，交回的、写好的行、报的错都和原来
/// 一样。
#[tokio::test]
async fn every_after_and_upto_reads_like_the_old_whole_walk() {
    let scratch = Scratch::new("places");
    let dir = scratch.0.join("s");
    let mut last = lines(71, 110);
    last.push_str(r#"{"seq":111,"at""#);
    write(
        &dir,
        &[
            (1, lines(1, 40)),
            (41, lines(41, 70)),
            (55, String::new()),
            (71, last),
        ],
    );
    let ends = [1, 2, 39, 40, 41, 42, 69, 70, 71, 72, 109, 110];
    for after in 0..=111 {
        for upto in ends.iter().copied().chain([after + 1, after + 4]) {
            if upto <= 110 {
                same_as_old(&dir, after, upto).await;
            }
        }
    }
}

/// 一段的分叉出来的会话（第一段从第 9 条起）、一段几千条的：照样一样。
#[tokio::test]
async fn a_forked_log_and_a_long_segment_read_alike() {
    let scratch = Scratch::new("forked");
    let dir = scratch.0.join("s");
    write(&dir, &[(9, lines(9, 30)), (31, lines(31, 4000))]);
    for after in [0, 1, 8, 9, 10, 29, 30, 31, 1999, 3998, 3999] {
        for upto in [10, 30, 31, 2000, 3999, 4000] {
            same_as_old(&dir, after, upto).await;
        }
    }
}

/// `upto` 在第一段中间的：读到它那一块就停，后面的段不打开（第二段是个打不开的目录，读到它就出错；原来读到最后一段末尾，
/// 在这里报错）。
#[tokio::test]
async fn upto_in_the_first_segment_leaves_later_segments_unopened() {
    let scratch = Scratch::new("unopened");
    let dir = scratch.0.join("s");
    write(&dir, &[(1, lines(1, 40))]);
    fs::create_dir_all(dir.join("000000000041.jsonl")).expect("建得了");
    assert!(old(&dir, 0, 20).is_err(), "原来读到第二段出错");
    for (after, upto) in [(0, 20), (0, 40), (19, 20), (5, 39), (39, 40)] {
        let read = backlog(&dir, after, upto).read().await.expect("读得了");
        let seqs: Vec<u64> = read.iter().map(|event| event.seq.get()).collect();
        assert_eq!(seqs, (after + 1..=upto).collect::<Vec<_>>());
        let lines = backlog(&dir, after, upto)
            .lines(written)
            .await
            .expect("读得了");
        assert_eq!(lines, read.iter().map(written).collect::<Vec<_>>());
    }
    // 读到第二段的照样报错，在交第一条以前。
    assert!(backlog(&dir, 30, 41).read().await.is_err());
    assert!(backlog(&dir, 30, 41).lines(written).await.is_err());
}

/// 坏的一行在要补的那一截里：报的错和原来从头读报的一字不差（哪一段、第几行）。在 `after` 以前的不解、不报，原来从头读要报
/// （项目主人定的差别，2026-11-03）；在 `upto` 以后的不读、不报。
#[tokio::test]
async fn a_broken_line_inside_is_told_alike_and_one_skipped_is_not_read() {
    let scratch = Scratch::new("broken");
    let dir = scratch.0.join("s");
    let second = lines(41, 70).replacen(&line(50), "{\"seq\":50,\"at\":\n", 1);
    write(
        &dir,
        &[(1, lines(1, 40)), (41, second), (71, lines(71, 90))],
    );
    for (after, upto) in [(0, 90), (40, 60), (49, 50), (45, 52), (0, 50)] {
        same_as_old(&dir, after, upto).await;
        let why = backlog(&dir, after, upto)
            .read()
            .await
            .expect_err("坏的那一行在里面");
        assert_eq!(why, "000000000041.jsonl line 10 is broken");
    }
    for (after, upto) in [(50, 90), (60, 70), (0, 49), (10, 40)] {
        assert!(old(&dir, after, upto).is_err(), "原来从头读都报");
        let read = backlog(&dir, after, upto).read().await.expect("读得了");
        let seqs: Vec<u64> = read.iter().map(|event| event.seq.get()).collect();
        assert_eq!(seqs, (after + 1..=upto).collect::<Vec<_>>());
    }
}

/// 没有要补的（`after` 不比 `upto` 小）不读日志：会话目录没有也不报。
#[tokio::test]
async fn nothing_to_replay_reads_nothing() {
    let scratch = Scratch::new("nothing");
    let dir = scratch.0.join("missing");
    for (after, upto) in [(0, 0), (5, 5), (7, 3), (u64::MAX, 3)] {
        assert_eq!(backlog(&dir, after, upto).read().await, Ok(Vec::new()));
        assert_eq!(
            backlog(&dir, after, upto).lines(written).await,
            Ok(Vec::new())
        );
    }
    assert!(backlog(&dir, 0, 1).read().await.is_err(), "有要补的才读");
}

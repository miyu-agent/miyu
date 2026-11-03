//! 只解要的种类（施工 V-39 上补，`read_range_only`）：交出去的就是每一行照 `Event::from_line_only` 读成的，序号、先后、
//! 停在哪同 `read_range`；坏了的报一样的错（哪一段、第几行、为什么），只有不要的种类的正文合 JSON、不合写法的不报。
//! 大到几个线程一起解的段也一样。

use std::path::Path;

use miyu_kernel::event::Event;

use super::{seq, write};
use crate::log::{Flow, OpenError, read_range, read_range_only};
use crate::test_support::Scratch;

/// 要的种类。
const ONLY: &[&str] = &["venue.delivered"];

/// 第 `seq` 条：照序号轮着是人话、工具结果、回复、发进群里的、不认识的种类，正文长短照序号变。
fn line(n: u64) -> String {
    let text = "字".repeat(usize::try_from(n % 17).unwrap_or(0) * 9);
    let (kind, body) = match n % 5 {
        0 => (
            "message.user",
            format!(r#"{{"blocks":[{{"type":"text","text":"第 {n} {text}"}}]}}"#),
        ),
        1 => (
            "tool.result",
            format!(
                r#"{{"call_id":"call_{n}_1","status":"ok","blocks":[{{"type":"text","text":"{text}"}}]}}"#
            ),
        ),
        2 => (
            "message.assistant",
            format!(r#"{{"blocks":[{{"type":"thinking","text":"{text}"}}],"seen":{n}}}"#),
        ),
        3 => (
            "venue.delivered",
            format!(
                r#"{{"line":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","turn":7,"to":[],"msg":"m{n}","text":"{text}"}}"#
            ),
        ),
        _ => ("ext.note", format!(r#"{{"text":"{text}"}}"#)),
    };
    format!(
        r#"{{"seq":{n},"at":"2026-09-25T07:00:00.000Z","kind":"{kind}","turn":7,"by":{{"kind":"kernel"}},"body":{body}}}"#
    )
}

/// 第 `from` 到第 `to` 条，一行一条；`change` 改写某几条。
fn lines(from: u64, to: u64, change: &dyn Fn(u64, String) -> String) -> Vec<u8> {
    (from..=to)
        .flat_map(|n| format!("{}\n", change(n, line(n))).into_bytes())
        .collect()
}

/// 照原样。
fn same(_: u64, line: String) -> String {
    line
}

/// 两种读法交出去的：只解要的种类的、全解的。
fn both(
    dir: &Path,
    from: u64,
    upto: Option<u64>,
) -> (Result<Vec<Event>, OpenError>, Result<Vec<Event>, OpenError>) {
    let mut only = Vec::new();
    let a = read_range_only(dir, seq(from), upto.map(seq), ONLY, |events| {
        only.extend(events);
        Flow::Go
    })
    .map(|()| only);
    let mut full = Vec::new();
    let b = read_range(dir, seq(from), upto.map(seq), |events| {
        full.extend(events);
        Flow::Go
    })
    .map(|()| full);
    (a, b)
}

/// 每一条照原文只解要的种类读成的。
fn expected(from: u64, to: u64, change: &dyn Fn(u64, String) -> String) -> Vec<Event> {
    (from..=to)
        .map(|n| Event::from_line_only(&change(n, line(n)), ONLY).expect("读得成"))
        .collect()
}

#[test]
fn every_range_reads_each_line_as_from_line_only() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    let mut last = lines(61, 90, &same);
    last.extend_from_slice(br#"{"seq":91,"at""#);
    write(
        &dir,
        &[
            (1, lines(1, 40, &same)),
            (41, lines(41, 60, &same)),
            (61, last),
        ],
    );
    for from in [1, 2, 40, 41, 59, 61, 90] {
        for upto in [None, Some(from), Some(from + 7)] {
            let (only, full) = both(&dir, from, upto);
            let (only, full) = (only.expect("读得成"), full.expect("读得成"));
            let to = upto.map_or(90, |upto| upto.min(90));
            assert_eq!(only, expected(from, to, &same), "从 {from} 到 {upto:?}");
            let seqs = |events: &[Event]| events.iter().map(|event| event.seq).collect::<Vec<_>>();
            assert_eq!(seqs(&only), seqs(&full));
        }
    }
}

/// 一段大到几个线程一起解（过 1 MiB）：照样每一行照 `from_line_only`，坏行的行号照段的开头数。
#[test]
fn a_big_segment_parsed_in_threads_reads_the_same() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    let wide = |_: u64, line: String| line.replace("字", &"字".repeat(4));
    write(&dir, &[(1, lines(1, 6000, &wide))]);
    let (only, _) = both(&dir, 1, None);
    assert_eq!(only.expect("读得成"), expected(1, 6000, &wide));
    let broken = |n: u64, line: String| {
        if n == 4321 {
            line.replacen(':', ";", 1)
        } else {
            wide(n, line)
        }
    };
    write(&dir, &[(1, lines(1, 6000, &broken))]);
    match both(&dir, 1, None) {
        (
            Err(OpenError::Broken {
                line: a, why: x, ..
            }),
            Err(OpenError::Broken {
                line: b, why: y, ..
            }),
        ) => {
            assert_eq!((a, &x), (b, &y));
            assert_eq!(a, 4321);
        }
        other => panic!("都该报坏了：{other:?}"),
    }
}

/// 改写某几条。
type Change = Box<dyn Fn(u64, String) -> String>;

/// 坏了的各样：报的错一样（哪一段、第几行、为什么）；不要的种类正文合 JSON、不合写法的，只解要的种类的不报。
#[test]
fn a_broken_log_is_refused_the_same_unless_a_skipped_body_is_still_json() {
    let scratch = Scratch::new();
    let refused: [(&str, Change); 7] = [
        // 第 21 条是工具结果（不要的）：外壳坏了。
        (
            "工具结果的序号写成字",
            Box::new(|n, line| {
                if n == 21 {
                    line.replacen("\"seq\":21", "\"seq\":\"21\"", 1)
                } else {
                    line
                }
            }),
        ),
        (
            "工具结果的时刻不对",
            Box::new(|n, line| {
                if n == 21 {
                    line.replace("2026-09-25T07:00:00.000Z", "昨天")
                } else {
                    line
                }
            }),
        ),
        // 不要的种类的正文不是合法 JSON。
        (
            "工具结果正文坏了",
            Box::new(|n, line| {
                if n == 21 {
                    line.replace("\"status\":\"ok\"", "\"status\":ok")
                } else {
                    line
                }
            }),
        ),
        // 要的种类的正文不合写法（第 20 条是人话、第 23 条是发进群里的）。
        (
            "人话正文不合写法",
            Box::new(|n, line| {
                if n == 20 {
                    line.replace("\"type\":\"text\"", "\"type\":5")
                } else {
                    line
                }
            }),
        ),
        (
            "发进群里的没写编号",
            Box::new(|n, line| {
                if n == 23 {
                    line.replace(",\"msg\":\"m23\"", "")
                } else {
                    line
                }
            }),
        ),
        // 只解几格的那几格不对（第 22 条是回复）。
        (
            "回复的 seen 是 0",
            Box::new(|n, line| {
                if n == 22 {
                    line.replace("\"seen\":22", "\"seen\":0")
                } else {
                    line
                }
            }),
        ),
        (
            "序号断了",
            Box::new(|n, line| {
                if n == 24 {
                    line.replace("\"seq\":24", "\"seq\":25")
                } else {
                    line
                }
            }),
        ),
    ];
    for (k, (what, change)) in refused.iter().enumerate() {
        let dir = scratch.path().join(format!("b{k}"));
        write(
            &dir,
            &[
                (1, lines(1, 10, &same)),
                (11, lines(11, 40, change.as_ref())),
            ],
        );
        // 从坏行后面读起的，跳过的那一截两样都不自检。
        for from in [1, 15, 21, 30] {
            match both(&dir, from, None) {
                (
                    Err(OpenError::Broken {
                        segment: s,
                        line: a,
                        why: x,
                    }),
                    Err(OpenError::Broken {
                        segment: t,
                        line: b,
                        why: y,
                    }),
                ) => {
                    assert_eq!((s, a, x), (t, b, y), "{what} 从 {from}");
                }
                (Ok(only), Ok(full)) if from > 15 => {
                    assert_eq!(only.len(), full.len(), "{what} 从 {from}")
                }
                other => panic!("{what} 从 {from} 都该报坏了：{other:?}"),
            }
        }
    }
    // 不是最后一段的末尾半行、段名不对：同样报。
    let dir = scratch.path().join("half");
    let mut first = lines(1, 10, &same);
    first.extend_from_slice(br#"{"seq":11"#);
    write(&dir, &[(1, first), (11, lines(11, 20, &same))]);
    let (a, b) = both(&dir, 1, None);
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert!(a.is_err());
    // 不要的种类（工具结果、回复）正文合 JSON、不合写法：全解报坏了，只解要的种类的照读。
    let dir = scratch.path().join("json");
    let typed = |n: u64, line: String| match n {
        21 => line.replace("\"status\":\"ok\"", "\"status\":5"),
        22 => line.replace("\"blocks\":[", "\"blocks\":7,\"x\":["),
        _ => line,
    };
    write(
        &dir,
        &[(1, lines(1, 20, &typed)), (21, lines(21, 40, &typed))],
    );
    let (only, full) = both(&dir, 1, None);
    assert!(
        matches!(full, Err(OpenError::Broken { line: 1, .. })),
        "第二段第一行（第 21 条）：{full:?}"
    );
    assert_eq!(only.expect("不解的不查"), expected(1, 40, &typed));
}

//! 攒批：窗口里的合成一批、隔得久的分开、半行攒满算一行、结束时剩下的照交；边读边截的和整份截的一字不差。

use std::sync::mpsc;
use std::thread;

use miyu_kernel::template::Template;

use super::*;

/// 截法：最多 `chars` 个字，省略那一行写 `[{count} cut]`。
fn cut(chars: usize) -> Reports {
    Reports {
        chars,
        omitted: Template::parse("[{count} cut]\n").expect("合写法"),
    }
}

/// 照先后交 `chunks`，每段之前先等给的毫秒数；交回攒出来的几批。
fn batches(chunks: Vec<(u64, &'static str)>, limit: usize) -> Vec<Batch> {
    let (sending, chunked) = mpsc::channel();
    thread::spawn(move || {
        for (wait, chunk) in chunks {
            thread::sleep(Duration::from_millis(wait));
            if sending.send(chunk.to_string()).is_err() {
                return;
            }
        }
    });
    let mut out = Vec::new();
    run(Box::new(chunked.into_iter()), &cut(limit), |batch| {
        out.push(batch);
    });
    out
}

#[test]
fn lines_inside_the_window_are_one_batch() {
    let got = batches(vec![(0, "one\ntwo\n"), (20, "three\n")], 1000);
    assert_eq!(
        got,
        [Batch {
            text: "one\ntwo\nthree".to_string(),
            lines: 3,
            omitted: 0
        }]
    );
}

#[test]
fn lines_further_apart_than_the_window_are_two_batches() {
    let wait = u64::try_from(WINDOW.as_millis()).expect("几百毫秒") * 3;
    let got = batches(vec![(0, "first\n"), (wait, "second\n")], 1000);
    let texts: Vec<&str> = got.iter().map(|batch| batch.text.as_str()).collect();
    assert_eq!(texts, ["first", "second"]);
}

#[test]
fn a_half_line_waits_for_its_end_or_the_end() {
    let got = batches(vec![(0, "par"), (10, "tial\nleft")], 1000);
    assert_eq!(
        got,
        [Batch {
            text: "partial\nleft".to_string(),
            lines: 2,
            omitted: 0
        }],
        "接上了才算一行；结束时没换行的那半行也交"
    );
}

#[test]
fn a_long_half_line_counts_as_a_line() {
    let long: &'static str = Box::leak(format!("{}y\n", "x".repeat(HALF_LINE)).into_boxed_str());
    let got = batches(vec![(0, long)], HALF_LINE * 2);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].lines, 2, "攒满的那一截算一行，后面的 y 另起一行");
    assert!(got[0].text.ends_with("x\ny"));
}

#[test]
fn a_flood_is_cut_the_same_as_cutting_it_whole() {
    let lines: Vec<String> = (0..3000).map(|n| format!("line {n}")).collect();
    let whole = lines.join("\n");
    let joined: &'static str = Box::leak(format!("{whole}\n").into_boxed_str());
    let limit = 100;
    let got = batches(vec![(0, joined)], limit);
    assert_eq!(got.len(), 1);
    let (expected, shortened) = cut(limit).cut(&whole);
    assert!(shortened);
    assert_eq!(got[0].text, expected, "边读边截和整份截一字不差");
    assert_eq!(got[0].lines, 3000);
    assert_eq!(
        got[0].omitted,
        u64::try_from(whole.chars().count() - limit).expect("不大")
    );
}

#[test]
fn a_batch_just_over_the_limit_is_cut_like_a_report() {
    let text: &'static str = Box::leak(format!("{}\n", "y".repeat(150)).into_boxed_str());
    let got = batches(vec![(0, text)], 100);
    let (expected, _) = cut(100).cut(&"y".repeat(150));
    assert_eq!(got[0].text, expected);
    assert_eq!(got[0].omitted, 50);
}

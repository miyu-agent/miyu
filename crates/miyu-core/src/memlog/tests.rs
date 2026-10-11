//! 什么时候采、一行长什么样。

use super::*;

fn timing() -> Timing {
    Timing {
        every: Duration::from_secs(60),
        peek: Duration::from_secs(5),
        grew: 32 * 1024 * 1024,
    }
}

#[test]
fn a_sample_is_due_on_the_minute_or_when_it_grew_fast() {
    let mib = 1024 * 1024;
    let t = timing();
    assert_eq!(
        due(Duration::from_secs(60), 100 * mib, Some(100 * mib), &t),
        Some(Why::Tick)
    );
    assert_eq!(
        due(Duration::from_secs(61), 100 * mib, None, &t),
        Some(Why::Tick),
        "看不到 RSS 也到点"
    );
    assert_eq!(
        due(Duration::from_secs(10), 100 * mib, Some(132 * mib), &t),
        Some(Why::Grew)
    );
    assert_eq!(
        due(Duration::from_secs(10), 100 * mib, Some(131 * mib), &t),
        None,
        "没涨够"
    );
    assert_eq!(
        due(Duration::from_secs(10), 100 * mib, Some(50 * mib), &t),
        None,
        "落了不算"
    );
    assert_eq!(due(Duration::from_secs(10), 100 * mib, None, &t), None);
    assert_eq!(
        due(Duration::ZERO, u64::MAX - 1, Some(u64::MAX - 1), &t),
        None,
        "加不溢出"
    );
}

#[test]
fn a_line_has_its_fields_in_order() {
    let at = Timestamp::parse("2026-10-11T05:32:08.270Z").expect("合写法");
    let usage = Usage {
        rss: 50,
        pss: Some(40),
        private: Some(30),
        anon: Some(20),
        swap: Some(0),
        footprint: None,
    };
    let heap = Stats {
        arenas: 9,
        mmapped: 8,
        used: 7,
        free: 2,
    };
    assert_eq!(
        line(at, Why::Start, &usage, Some(&heap), 3),
        r#"{"anon":20,"at":"2026-10-11T05:32:08.270Z","heap":{"arenas":9,"free":2,"mmapped":8,"used":7},"private":30,"pss":40,"rss":50,"sessions":3,"swap":0,"why":"start"}"#
    );
    assert_eq!(
        line(at, Why::Grew, &usage, None, 0),
        r#"{"anon":20,"at":"2026-10-11T05:32:08.270Z","private":30,"pss":40,"rss":50,"sessions":0,"swap":0,"why":"grew"}"#,
        "没有堆的统计不写 heap"
    );
    let mac = Usage {
        rss: 50,
        footprint: Some(45),
        ..Usage::default()
    };
    assert_eq!(
        line(at, Why::Tick, &mac, None, 1),
        r#"{"at":"2026-10-11T05:32:08.270Z","footprint":45,"rss":50,"sessions":1,"why":"tick"}"#,
        "macOS 上量得出的只有 rss 和 footprint，别的不写"
    );
    assert_eq!(Why::Tick.as_str(), "tick");
}

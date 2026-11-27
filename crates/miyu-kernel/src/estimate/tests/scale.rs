//! 估算和真值的倍数（施工 CT-2，`compaction.md` 第一条第 4 款）：怎么挑、怎么夹、用量里怎么乘。

use super::*;

/// 一次说完了、带着估算的主请求：真值 `usage`、发出去时整份的本地估算 `estimate`（施工 CT-2）。
fn estimated(seq: u64, usage: u64, estimate: u64) -> Event {
    let body = format!(
        r#"{{"seen":{},"endpoint":"deepseek","model":"deepseek-flash","messages":1,"estimate":{estimate},"usage":{{"uncached":{usage},"cache_read":0,"cache_write":0,"output":0}},"result":"ok"}}"#,
        seq - 1
    );
    event(seq, Some(2), KERNEL, "model.called", &body)
}

/// 倍数照最近一次带着估算的主请求（施工 CT-2）：真值除以估算，千分之几；夹在一倍到三倍之间（报得比估算少的照估算）；没有带估算的是一倍；
/// 别的模型的、没带估算的（以前的日志）、估算不到 [`SCALE_FROM`] 的小请求不算。
#[test]
fn the_scale_is_the_latest_reported_over_estimated() {
    let head = || {
        vec![
            event(1, None, ALICE, "message.user", r#"{"blocks":[]}"#),
            event(2, Some(2), KERNEL, "turn.started", r#"{"trigger":1}"#),
        ]
    };
    let scale_of = |mut more: Vec<Event>| {
        let mut events = head();
        events.append(&mut more);
        scale(&history(events), &flash()).per_mille()
    };
    assert_eq!(scale_of(vec![]), 1000, "没有可比的：一倍");
    assert_eq!(
        scale_of(vec![called(3, 2, 1, Some(100), "ok")]),
        1000,
        "以前的日志没有估算"
    );
    // 2026-11-27 主人的群会话：估 41177，agy 报 76350。
    assert_eq!(scale_of(vec![estimated(3, 76350, 41177)]), 1854);
    // 最近的那一次算数；后面没带估算的、估算太小的不挡。
    assert_eq!(
        scale_of(vec![
            estimated(3, 40_000, 20_000),
            estimated(4, 30_000, 20_000),
            called(5, 2, 1, Some(999), "ok"),
            estimated(6, 9_000, 1_000),
        ]),
        1500
    );
    assert_eq!(
        scale_of(vec![estimated(3, 9_000, 1_000)]),
        1000,
        "小请求不定倍数"
    );
    assert_eq!(
        scale_of(vec![estimated(3, 100_000, 20_000)]),
        SCALE_MOST,
        "至多三倍"
    );
    assert_eq!(
        scale_of(vec![estimated(3, 1_000, 20_000)]),
        SCALE_LEAST,
        "至少一倍：报得比估算少的照估算"
    );
    let other = r#"{"seen":2,"endpoint":"other","model":"m","messages":1,"estimate":20000,"usage":{"uncached":60000,"cache_read":0,"cache_write":0,"output":0},"result":"ok"}"#;
    assert_eq!(
        scale_of(vec![event(3, Some(2), KERNEL, "model.called", other)]),
        1000,
        "别的模型的不算"
    );
}

/// 用量里的估算乘上倍数（施工 CT-2）：没有锚的整份乘，有锚的锚以后新加的乘，锚本身是真值、不乘。
#[test]
fn usage_scales_the_estimated_part_only() {
    let now = request(
        "sys",
        vec![
            user("hello"),
            assistant("hi"),
            user("abcd"),
            user("abcdefgh"),
        ],
    );
    let plain = usage(&now, None, &flash(), &FLAT, Scale::ONE);
    assert_eq!(usage(&now, None, &flash(), &FLAT, Scale(2000)), plain * 2);
    // 锚 1000 是真值；锚以后新加的 1 + 2 乘两倍。
    assert_eq!(
        usage(&now, Some(&anchored(1, 1000)), &flash(), &FLAT, Scale(2000)),
        1000 + 6
    );
}

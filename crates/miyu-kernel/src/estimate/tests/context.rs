//! 驱动另报的上下文大小（施工 ACP-1，设计 73 第三节）。

use super::*;

#[test]
fn a_reported_context_size_is_the_anchor_instead_of_the_summed_usage() {
    // 扩展驱动一次调用里来回请求了三次：四项加起来 106000，上下文其实只有最后一次的 50000。
    let body = r#"{"seen":2,"endpoint":"deepseek","model":"deepseek-flash","messages":1,"usage":{"uncached":96000,"cache_read":0,"cache_write":0,"output":10000,"context":50000},"result":"ok"}"#;
    let history = history(vec![event(3, Some(2), KERNEL, "model.called", body)]);
    assert_eq!(anchor(&history).map(|a| a.reported), Some(50_000));
}

#[test]
fn the_context_size_survives_the_log_and_old_lines_stay_byte_for_byte() {
    let with = r#"{"uncached":1,"cache_read":2,"cache_write":3,"output":4,"context":7}"#;
    let usage: crate::event::Usage = serde_json::from_str(with).unwrap();
    assert_eq!(usage.context, Some(7));
    assert_eq!(serde_json::to_string(&usage).unwrap(), with);
    let without = r#"{"uncached":1,"cache_read":2,"cache_write":3,"output":4}"#;
    let usage: crate::event::Usage = serde_json::from_str(without).unwrap();
    assert_eq!(
        serde_json::to_string(&usage).unwrap(),
        without,
        "旧日志照原样写回"
    );
}

#[test]
fn adding_usages_keeps_the_latest_context_size_instead_of_adding_it() {
    let one = crate::event::Usage {
        uncached: 1,
        cache_read: 0,
        cache_write: 0,
        output: 1,
        reasoning: None,
        context: Some(10),
    };
    let later = crate::event::Usage {
        context: Some(30),
        ..one
    };
    let silent = crate::event::Usage {
        context: None,
        ..one
    };
    assert_eq!(one.plus(&later).context, Some(30));
    assert_eq!(one.plus(&silent).context, Some(10));
    assert_eq!(one.plus(&later).uncached, 2, "记账的四项照旧加起来");
}

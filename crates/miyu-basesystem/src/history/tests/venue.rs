//! 补拉回来的群消息（施工 O-40，`docs/blueprint/tools/history.md`「时刻」）：平台给了时刻的照它显示、照它筛，序号照记下的先后。

use super::*;

/// 第 1 条 05:01 记下；第 2 条 05:05 才补回来，平台说是 04:30 发的。
fn backfilled() -> Vec<Vec<Event>> {
    let said = |seq: u64, at: &str, text: &str, venue: Value| {
        event(
            seq,
            at,
            "message.user",
            None,
            alice(),
            json!({"blocks": [{"type": "text", "text": text}], "venue": venue}),
        )
    };
    vec![vec![
        said(
            1,
            "2026-09-29T05:01:00.000Z",
            "早",
            json!({"msg": "8801", "ambient": true}),
        ),
        said(
            2,
            "2026-09-29T05:05:00.000Z",
            "断线时说的",
            json!({"msg": "8799", "ambient": true, "backfill": true, "sent": "2026-09-29T04:30:00.000Z"}),
        ),
    ]]
}

#[tokio::test]
async fn a_backfilled_message_shows_when_it_was_sent() {
    let done = run_on(backfilled(), json!({})).await;
    assert_eq!(
        text(&done),
        "#1 2026-09-29 14:01 user\n早\n\n#2 2026-09-29 13:30 user\n断线时说的\n"
    );
    // 筛的时刻也照平台给的。
    let early = run_on(backfilled(), json!({"until": "2026-09-29 13:45"})).await;
    assert_eq!(seqs(&early), [2]);
}

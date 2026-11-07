//! 扩展驱动那两项（施工 MD-4）：认得出哪一条推送是这个会话的一段正文增量。

use serde_json::json;

use super::*;

#[test]
fn only_a_text_delta_of_this_session_counts() {
    let push = |session: &str, kind: &str, body: Value| json!({"method": "event", "params": {"session": session, "event": {"kind": kind, "body": body}}});
    let text = json!({"index": 0, "text": "ok"});
    assert!(is_text_delta(
        &push("s1", "model.delta", text.clone()),
        "s1"
    ));
    assert!(
        !is_text_delta(
            &push("s1", "model.delta", json!({"index": 0, "start": "text"})),
            "s1"
        ),
        "开块不算"
    );
    assert!(
        !is_text_delta(&push("s2", "model.delta", text), "s1"),
        "别的会话"
    );
    assert!(!is_text_delta(&push("s1", "turn.ended", json!({})), "s1"));
}

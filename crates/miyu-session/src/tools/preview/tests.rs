//! 问人之前看的那一眼放进说明（施工 F-10 下）：原来的几格留着，多一格 `preview`；原来没有说明的只有这一格。

use serde_json::json;

use super::*;

fn raw(value: &Value) -> RawJson {
    serde_json::from_str(&value.to_string()).expect("读得回来")
}

#[test]
fn the_preview_joins_what_the_guard_wrote() {
    let detail = raw(&json!({"tool": "package", "install": "/w/kit"}));
    let merged = with_preview(Some(detail), json!({"package": "kit", "files": 2}));
    assert_eq!(
        serde_json::from_str::<Value>(merged.get()).expect("是 JSON"),
        json!({"tool": "package", "install": "/w/kit", "preview": {"package": "kit", "files": 2}})
    );
    let alone = with_preview(None, json!({"package": "kit"}));
    assert_eq!(alone.get(), r#"{"preview":{"package":"kit"}}"#);
}

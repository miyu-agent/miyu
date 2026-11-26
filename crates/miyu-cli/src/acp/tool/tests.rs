use serde_json::{Value, json};

use super::{changed, diffs, kind, opened, title};

/// 一步工具，照视图流里的写法。
fn step(fields: Value) -> miyu_view::Tool {
    let mut base = json!({"group": "g1", "name": "read", "title": {"name": "读取"}, "state": "running",
        "args": "{\"file_path\":\"notes.md\"}"});
    if let (Some(base), Some(fields)) = (base.as_object_mut(), fields.as_object()) {
        base.extend(fields.clone());
    }
    serde_json::from_value(base).expect("是一步工具")
}

#[test]
fn kinds_follow_the_tool() {
    let cases = [
        ("read", "read"),
        ("edit", "edit"),
        ("write", "edit"),
        ("trash", "delete"),
        ("grep", "search"),
        ("glob", "search"),
        ("WebSearch", "search"),
        ("shell", "execute"),
        ("web_fetch", "fetch"),
        ("todowrite", "think"),
        ("subagent", "other"),
    ];
    for (name, wanted) in cases {
        assert_eq!(kind(name), wanted, "{name}");
    }
}

#[test]
fn the_title_is_the_card_line() {
    let tool = step(
        json!({"title": {"name": "网络搜索", "object": "miyu", "via": "Exa", "said": "5 条"}}),
    );
    assert_eq!(title(&tool), "网络搜索 (Exa) miyu · 5 条");
    assert_eq!(title(&step(json!({}))), "读取");
}

#[test]
fn a_new_step_is_pending_until_its_arguments_are_written() {
    let preparing = step(json!({"state": "preparing", "args": "{\"file_pa"}));
    let call = opened("b3.1", &preparing, "/work");
    assert_eq!(call["status"], "pending");
    assert!(call.get("rawInput").is_none(), "写到一半的参数不报：{call}");
    let running = step(json!({}));
    let call = opened("b3.1", &running, "/work");
    assert_eq!(call["status"], "in_progress");
    assert_eq!(call["rawInput"]["file_path"], "notes.md");
    // 相对的路径照工作目录补全。
    let path = std::path::Path::new("/work").join("notes.md");
    assert_eq!(
        call["locations"][0]["path"],
        path.to_string_lossy().as_ref()
    );
}

#[test]
fn a_step_waiting_for_approval_is_pending() {
    let waiting = step(json!({"approval": {"access": "execute"}}));
    let call = opened("b3.1", &waiting, "/work");
    assert_eq!(call["status"], "pending", "{call}");
}

#[test]
fn an_update_carries_only_what_changed() {
    let running = step(json!({}));
    assert_eq!(changed("b3.1", &running, &running, "/work"), None);
    let done = step(
        json!({"state": "ok", "output": "一\n二\n", "title": {"name": "读取", "said": "2 行"}}),
    );
    let update = changed("b3.1", &running, &done, "/work").expect("变了");
    assert_eq!(update["status"], "completed");
    assert_eq!(update["title"], "读取 · 2 行");
    assert_eq!(update["content"][0]["content"]["text"], "```\n一\n二\n```");
    let denied =
        step(json!({"state": "denied", "title": {"name": "读取", "said": "只读模式，未执行"}}));
    let update = changed("b3.1", &running, &denied, "/work").expect("变了");
    assert_eq!(update["status"], "failed");
    assert_eq!(update["content"][0]["content"]["text"], "只读模式，未执行");
}

#[test]
fn output_with_backticks_gets_a_longer_fence() {
    let done = step(json!({"state": "ok", "output": "a ``` b"}));
    let update = changed("b3.1", &step(json!({})), &done, "/work").expect("变了");
    assert_eq!(
        update["content"][0]["content"]["text"],
        "````\na ``` b\n````"
    );
}

#[test]
fn unified_diffs_become_before_and_after_pieces() {
    let detail = json!({"files": [
        {"path": "/w/a.md", "action": "write", "added": 1, "removed": 1,
         "diff": ["@@ -1,3 +1,3 @@", " 一", "-二", "+贰", " 三", "@@ -9,1 +9,2 @@", " 九", "+十"]},
        {"path": "/w/new.md", "action": "write", "added": 2, "removed": 0,
         "diff": "--- /dev/null\n+++ b/new.md\n@@ -0,0 +1,2 @@\n+甲\n+乙\n"},
        {"path": "/w/big.bin", "action": "write", "skipped": "binary"},
    ]});
    let pieces = diffs(&detail);
    assert_eq!(
        pieces,
        [
            json!({"type": "diff", "path": "/w/a.md", "oldText": "一\n二\n三\n", "newText": "一\n贰\n三\n"}),
            json!({"type": "diff", "path": "/w/a.md", "oldText": "九\n", "newText": "九\n十\n"}),
            json!({"type": "diff", "path": "/w/new.md", "oldText": null, "newText": "甲\n乙\n"}),
        ]
    );
}

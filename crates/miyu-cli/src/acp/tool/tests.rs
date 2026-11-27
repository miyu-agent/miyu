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
fn a_path_alias_still_names_the_location() {
    // 施工 ED-3：`path`、`filePath` 这些别名的，照标准写法的 `file_path` 报位置。
    let args = json!({"path": "notes.md", "edits": {"oldText": "x", "newText": "y"}}).to_string();
    let call = opened(
        "b3.1",
        &step(json!({"name": "edit", "args": args})),
        "/work",
    );
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

#[test]
fn a_command_shows_its_text_above_the_output() {
    let args = "{\"command\":\"ls -la\",\"description\":\"List files\"}";
    let running = step(
        json!({"name": "shell", "title": {"name": "执行命令", "object": "List files"}, "args": args}),
    );
    let call = opened("b3.1", &running, "/work");
    assert_eq!(call["title"], "执行命令 List files", "标题照旧");
    assert_eq!(
        call["content"],
        json!([{"type": "content", "content": {"type": "text", "text": "```\n$ ls -la\n```"}}])
    );
    let done = step(
        json!({"name": "shell", "title": {"name": "执行命令", "object": "List files"}, "args": args,
        "state": "ok", "output": "a\n"}),
    );
    let update = changed("b3.1", &running, &done, "/work").expect("变了");
    // 内容整份换：命令照样在最前面。
    assert_eq!(
        update["content"][0]["content"]["text"],
        "```\n$ ls -la\n```"
    );
    assert_eq!(update["content"][1]["content"]["text"], "```\na\n```");
    // 写到一半的参数不报命令。
    let preparing = step(json!({"name": "shell", "state": "preparing", "args": "{\"comm"}));
    assert!(opened("b3.1", &preparing, "/work").get("content").is_none());
}

#[test]
fn a_hosted_command_the_driver_asks_about_shows_its_text_like_a_shell() {
    // 施工 ACP-12：对方自带的工具（Claude 适配器的 `Bash`）经驱动问人，命令原文在确认的 `detail` 里；以前卡片只有标题，要点开
    // 参数原文才看得到要跑什么。
    let detail = json!({"driver": "acp/acp", "tool": "touch a.txt", "title": "touch a.txt",
        "command": "touch a.txt"});
    let asked = step(
        json!({"name": "Bash", "hosted": true, "title": {"name": "Bash", "via": "ACP 智能体"},
        "args": "{\"command\":\"touch a.txt\"}", "approval": {"access": "execute", "detail": detail}}),
    );
    let call = opened("b5.0", &asked, "/work");
    assert_eq!(call["title"], "Bash (ACP 智能体)");
    assert_eq!(call["status"], "pending");
    assert_eq!(
        call["content"],
        json!([{"type": "content", "content": {"type": "text", "text": "```\n$ touch a.txt\n```"}}])
    );
    // 包问人的那种（没有命令）、没问过的不报命令。
    let guarded = json!({"driver": "acp/acp", "guard": "acp", "tool": "Edit", "reason": "a.txt"});
    let edit = step(
        json!({"name": "Edit", "hosted": true, "title": {"name": "Edit"},
        "args": "{}", "approval": {"access": "write", "detail": guarded}}),
    );
    assert!(opened("b5.1", &edit, "/work").get("content").is_none());
    let unasked = step(
        json!({"name": "Bash", "hosted": true, "title": {"name": "Bash"},
        "args": "{\"command\":\"ls\"}"}),
    );
    assert!(opened("b5.2", &unasked, "/work").get("content").is_none());
}

//! `driver.approve` 的参数怎么读、内核的结论怎么写回（施工 ACP-8）。

use serde_json::{Value, json};

use super::*;

fn options() -> Value {
    json!([
        {"id": "yes", "name": "Allow", "kind": "allow_once"},
        {"id": "always", "name": "Always", "kind": "allow_always"},
        {"id": "no", "name": "Reject", "kind": "reject_once"}
    ])
}

fn detail_of(asked: &Asked) -> Value {
    serde_json::from_str(asked.detail.get()).unwrap()
}

#[test]
fn a_command_is_drawn_like_running_a_command() {
    let params = json!({"call": "d1", "index": 2, "access": "execute", "title": "List files",
        "command": "ls -la", "text": "ignored", "options": options()});
    let asked = read(&params, "acp", "acp/claude-acp").unwrap();
    assert_eq!((asked.index, &asked.access), (2, &Access::Execute));
    assert_eq!(
        detail_of(&asked),
        json!({"driver": "acp/claude-acp", "tool": "List files", "title": "List files", "command": "ls -la"})
    );
}

#[test]
fn anything_else_is_drawn_like_a_package_asking() {
    let params = json!({"index": 0, "access": "write", "title": "Edit src/main.rs",
        "text": "src/main.rs", "options": options()});
    let asked = read(&params, "acp", "acp/claude-acp").unwrap();
    assert_eq!(
        detail_of(&asked),
        json!({"driver": "acp/claude-acp", "guard": "acp", "tool": "Edit src/main.rs", "reason": "src/main.rs"})
    );
    let bare =
        json!({"index": 0, "access": "write", "title": "Edit", "text": "  ", "options": options()});
    assert!(
        detail_of(&read(&bare, "acp", "acp/x").unwrap())
            .get("reason")
            .is_none(),
        "空白的说明不写"
    );
}

#[test]
fn long_texts_are_cut() {
    let long = "字".repeat(TEXT_CHARS + 5);
    let params = json!({"index": 0, "access": "execute", "title": "t".repeat(300),
        "command": long, "options": options()});
    let detail = detail_of(&read(&params, "acp", "acp/x").unwrap());
    let command = detail["command"].as_str().unwrap();
    assert_eq!(command.chars().count(), TEXT_CHARS + 1);
    assert!(command.ends_with('…'));
    assert_eq!(
        detail["title"].as_str().unwrap().chars().count(),
        TITLE_CHARS + 1
    );
}

#[test]
fn bad_params_say_what_is_wrong() {
    let good = json!({"index": 0, "access": "execute", "title": "t", "options": options()});
    assert!(read(&good, "acp", "acp/x").is_ok());
    let broken = |key: &str, value: Value| {
        let mut params = good.clone();
        params[key] = value;
        read(&params, "acp", "acp/x").unwrap_err()
    };
    assert!(broken("index", json!(-1)).contains("index"));
    assert!(broken("access", json!("install")).contains("access"));
    assert!(broken("title", json!("")).contains("title"));
    assert!(broken("command", json!(3)).contains("command"));
    assert!(broken("options", json!([])).contains("options"));
    assert!(broken("options", json!([{"id": "x", "kind": "maybe"}])).contains("options"));
    let many: Vec<Value> = (0..17)
        .map(|n| json!({"id": n.to_string(), "kind": "allow_once"}))
        .collect();
    assert!(broken("options", json!(many)).contains("options"));
}

#[test]
fn answers_pick_the_matching_option() {
    let choices = read(
        &json!({"index": 0, "access": "execute", "title": "t", "options": options()}),
        "acp",
        "acp/x",
    )
    .unwrap()
    .choices;
    assert_eq!(
        answered(&DriverAnswer::Allowed, &choices).unwrap(),
        json!({"decision": "once", "option": "yes"})
    );
    let denied = DriverAnswer::Denied {
        reason: Some("no".to_string()),
    };
    assert_eq!(
        answered(&denied, &choices).unwrap(),
        json!({"decision": "deny", "option": "no", "reason": "no"})
    );
    assert_eq!(
        answered(&DriverAnswer::Auto(AutoDeny::Unattended), &choices).unwrap(),
        json!({"decision": "deny", "option": "no", "auto": "unattended"})
    );
    assert_eq!(
        answered(&DriverAnswer::Cancelled, &choices).unwrap(),
        json!({"decision": "cancelled"})
    );
    assert!(answered(&DriverAnswer::NotHosted, &choices).is_err());
}

#[test]
fn without_a_narrower_option_the_broader_one_is_picked_or_none() {
    let only = |options: Value| {
        read(
            &json!({"index": 0, "access": "execute", "title": "t", "options": options}),
            "acp",
            "acp/x",
        )
        .unwrap()
        .choices
    };
    let always =
        only(json!([{"id": "a", "kind": "allow_always"}, {"id": "r", "kind": "reject_always"}]));
    assert_eq!(
        answered(&DriverAnswer::Allowed, &always).unwrap(),
        json!({"decision": "once", "option": "a"})
    );
    assert_eq!(
        answered(&DriverAnswer::Denied { reason: None }, &always).unwrap(),
        json!({"decision": "deny", "option": "r"})
    );
    let allow_only = only(json!([{"id": "a", "kind": "allow_once"}]));
    assert_eq!(
        answered(&DriverAnswer::Denied { reason: None }, &allow_only).unwrap(),
        json!({"decision": "deny"}),
        "没有拒绝的选项不写"
    );
    assert_eq!(
        refused(&json!({"options": [{"id": "r", "kind": "reject_once"}]})).unwrap(),
        json!({"decision": "deny", "option": "r", "auto": "aside"})
    );
}

use serde_json::json;

use super::{Choices, applied, options, request};
use crate::acp::session::Current;
use crate::language::Language;

/// `model.list` 的回应：一家供应商两个模型（一个没密钥、一个带思考档位），一个能用的池、一个不能用的。
fn listed() -> serde_json::Value {
    json!({
        "providers": [{"id": "ds", "name": {"value": "DeepSeek"}, "models": [
            {"model": "flash", "ref": "ds/flash", "state": "ok",
             "facts": {"name": {"value": "DeepSeek Flash"}, "reasoning": {"value": ["low", "high"]}}},
            {"model": "pro", "ref": "ds/pro", "state": "no_key", "facts": {}},
        ]}],
        "pools": [{"name": "normal", "usable": true}, {"name": "lite", "usable": false}],
    })
}

#[test]
fn choices_are_models_with_keys_and_usable_pools() {
    let choices = Choices::of(&listed(), Language::Chinese);
    assert!(choices.offers("ds/flash"));
    assert!(!choices.offers("ds/pro"));
    assert!(choices.offers("@normal"));
    assert!(!choices.offers("@lite"));
    assert_eq!(choices.levels(Some("ds/flash")), ["low", "high"]);
    assert!(choices.levels(Some("@normal")).is_empty());
}

#[test]
fn options_are_mode_model_and_effort_in_that_order() {
    let choices = Choices::of(&listed(), Language::Chinese);
    let current = Current {
        mode: "full".to_string(),
        model: Some("ds/flash".to_string()),
        effort: Some("high".to_string()),
    };
    let shown = options(Language::Chinese, &choices, &current);
    let ids: Vec<&str> = shown
        .iter()
        .filter_map(|option| option["id"].as_str())
        .collect();
    assert_eq!(ids, ["mode", "model", "effort"]);
    assert_eq!(shown[1]["options"][0]["name"], "DeepSeek");
    assert_eq!(
        shown[1]["options"][0]["options"][0],
        json!({"value": "ds/flash", "name": "DeepSeek Flash"})
    );
    assert_eq!(shown[2]["category"], "thought_level");
    // 池没有思考档位；现在用的不在能选的里面的也列上。
    let pooled = Current {
        model: Some("gone/model".to_string()),
        ..current
    };
    let shown = options(Language::Chinese, &choices, &pooled);
    assert_eq!(shown.len(), 2);
    assert_eq!(shown[1]["options"][0]["options"][0]["value"], "gone/model");
}

#[test]
fn requests_check_the_value() {
    let choices = Choices::of(&listed(), Language::Chinese);
    let current = Current {
        mode: "full".to_string(),
        model: Some("ds/flash".to_string()),
        effort: None,
    };
    let (method, params) = request("s1", &choices, &current, "mode", "read-only").expect("认得");
    assert_eq!(method, "session.set_permission_level");
    assert_eq!(params, json!({"session": "s1", "read_only": true}));
    assert!(
        request("s1", &choices, &current, "model", "ds/pro").is_none(),
        "没密钥的不能选"
    );
    assert!(request("s1", &choices, &current, "effort", "max").is_none());
    let (_, params) = request("s1", &choices, &current, "effort", "low").expect("认得");
    assert_eq!(params, json!({"session": "s1", "effort": "low"}));
    assert_eq!(
        applied(&current, "effort", "low").effort.as_deref(),
        Some("low")
    );
}

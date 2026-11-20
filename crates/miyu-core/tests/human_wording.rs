//! 中文界面的用词（施工 WD-1，`internal/blueprint/config.md`「给人看的字」的「用词的规矩」）：中文字表里说到 API 密钥一律写「密钥」，
//! 不写「key」；不用「她」「他」称呼 AI。

use std::path::Path;

use serde_json::Value;

/// 这几句说的是配置里的键名 `key`（软件源的 `url`、`key` 两格），不是密钥：留着。
const FIELD_NAMES: [&str; 2] = ["said.config/source-incomplete", "said.config/source-key"];

/// 字表里的每一句和它的位置（`config.items.<键>.description`、`said.<编号>` 这样）。
fn texts(value: &Value, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                let below = match path {
                    "" => key.clone(),
                    _ => format!("{path}.{key}"),
                };
                texts(inner, &below, out);
            }
        }
        Value::Array(list) => {
            for inner in list {
                texts(inner, path, out);
            }
        }
        Value::String(text) => out.push((path.to_string(), text.clone())),
        _ => {}
    }
}

/// 去掉 `{名字}` 这样的占位以后，有没有单独成词的 `key`、`keys`（大小写不论）。连在名字里的不算（`providers.<id>.key`）。
fn says_key(text: &str) -> bool {
    let mut plain = String::new();
    let mut inside = false;
    for ch in text.chars() {
        match ch {
            '{' => inside = true,
            '}' => inside = false,
            _ if !inside => plain.push(ch),
            _ => {}
        }
    }
    let lower = plain.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let joined = |byte: Option<u8>| {
        byte.is_some_and(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    };
    lower.match_indices("key").any(|(at, _)| {
        let before = at.checked_sub(1).map(|at| bytes[at]);
        let end = match bytes.get(at + 3) {
            Some(b's') => at + 4,
            _ => at + 3,
        };
        !joined(before) && !joined(bytes.get(end).copied())
    })
}

/// 有没有「她」，或者不在「其他」里的「他」。
fn says_she_or_he(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    chars.iter().enumerate().any(|(at, ch)| match ch {
        '她' => true,
        '他' => at == 0 || chars[at - 1] != '其',
        _ => false,
    })
}

#[test]
fn chinese_words_say_miyao_and_never_she() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/core/human/zh.json");
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("读得到")).expect("是 JSON");
    let mut all = Vec::new();
    texts(&json, "", &mut all);
    assert!(all.len() > 500, "字表读全了：{}", all.len());
    let wrong: Vec<String> = all
        .iter()
        .filter(|(path, text)| {
            (says_key(text) && !FIELD_NAMES.contains(&path.as_str())) || says_she_or_he(text)
        })
        .map(|(path, text)| format!("{path}：{text}"))
        .collect();
    assert_eq!(wrong, Vec::<String>::new());
}

#[test]
fn the_word_checks_catch_what_they_should() {
    for text in ["要 key", "API Key 和地址", "{provider} 的 key", "几个 keys"] {
        assert!(says_key(text), "{text}");
    }
    for text in [
        "{key} 没有填写",
        "providers.<id>.key 的值",
        "密钥",
        "keyword",
        "monkey",
    ] {
        assert!(!says_key(text), "{text}");
    }
    assert!(says_she_or_he("她的名字"));
    assert!(says_she_or_he("他说的"));
    assert!(!says_she_or_he("其他软件包"));
}

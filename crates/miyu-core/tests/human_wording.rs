//! 中文界面的用词（施工 WD-1，`internal/blueprint/config.md`「给人看的字」的「用词的规矩」）：中文字表里说到 API 密钥一律写「密钥」，
//! 不写「key」；不用「她」「他」称呼 AI。施工 WD-4 起另外不出现施工时的口语和文言字眼（[`SLANG`]）。

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

/// 「要更新的 X」读得成「需要更新版本的 X」，也读得成「将要被更新的 X」（施工 WD-3：`package-problems/maybe_newer_miyu` 原来
/// 写「这个包可能要更新的 Miyu」）：中文字表里不这样写，写「需要更新版本的」。
#[test]
fn chinese_words_do_not_say_the_ambiguous_yao_gengxin_de() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/core/human/zh.json");
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("读得到")).expect("是 JSON");
    let mut all = Vec::new();
    texts(&json, "", &mut all);
    let wrong: Vec<String> = all
        .iter()
        .filter(|(_, text)| text.contains("要更新的"))
        .map(|(path, text)| format!("{path}：{text}"))
        .collect();
    assert_eq!(wrong, Vec::<String>::new());
}

/// 施工时随手写的口语、内部说法和文言字眼（施工 WD-4，`AGENTS.md`「文档的规矩」的词表）：中文字表里不出现，写成后面括号里的说法。
/// 只拦一眼看得出的词，句子是否完整仍然由人审。
const SLANG: [&str; 18] = [
    "不认识",   // 未知的
    "写法不对", // 格式不正确
    "登记",     // 注册
    "拦下",     // 已阻止
    "这一份",   // 这个版本、这个软件包
    "那一份",
    "写的是", // 当前为
    "没加上", // 未添加
    "起不来", // 无法启动
    "照旧",   // 仍然
    "印出",   // 输出
    "此会话", // 这个会话
    "此文件", // 这个文件
    "尚未",   // 还没有
    "无需",   // 不需要
    "如需",   // 如果需要
    "即可",   // 就可以
    "您",     // 你
];

#[test]
fn chinese_words_avoid_construction_slang() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/core/human/zh.json");
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("读得到")).expect("是 JSON");
    let mut all = Vec::new();
    texts(&json, "", &mut all);
    let wrong: Vec<String> = all
        .iter()
        .filter(|(_, text)| SLANG.iter().any(|word| text.contains(word)))
        .map(|(path, text)| format!("{path}：{text}"))
        .collect();
    assert_eq!(wrong, Vec::<String>::new());
}

/// 出厂软件包自己的字表（`resources/software/<包>/human/zh.json`）守同一张词表（施工 WD-5）：工具的结果句、报错和核心字表、
/// 拒绝原因表说同一套话。
#[test]
fn shipped_package_words_avoid_construction_slang() {
    let software = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software");
    let mut all = Vec::new();
    let mut tables = 0;
    for entry in std::fs::read_dir(&software).expect("读得到出厂软件包的目录") {
        let package = entry.expect("读得到这一项").file_name();
        let file = software.join(&package).join("human/zh.json");
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let json: Value = serde_json::from_str(&text).expect("是 JSON");
        texts(&json, &package.to_string_lossy(), &mut all);
        tables += 1;
    }
    assert!(tables >= 7, "出厂软件包的字表读全了：{tables}");
    let wrong: Vec<String> = all
        .iter()
        .filter(|(_, text)| SLANG.iter().any(|word| text.contains(word)))
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

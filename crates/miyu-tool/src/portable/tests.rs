//! 不是各家都收的参数写法（施工 FX-10）：整数、数字、真假上的 `enum`、`enum` 里不是字符串的值报出位置；字符串的不报；只走
//! schema 的位置；出厂的工具说明一件都没有；包文件夹里照 `tools/<名字>.json` 找。

use std::path::Path;

use serde_json::json;

use super::{Found, scan, unportable};

/// 接入通讯平台那一件（2026-11-20 撞上的）：`days` 是整数，`enum` 写了 1、7、30。
#[test]
fn an_enum_on_an_integer_is_found() {
    let schema = json!({
        "type": "object",
        "properties": {
            "days": {"type": "integer", "enum": [1, 7, 30]},
            "scope": {"type": "string", "enum": ["group", "all"]}
        }
    });
    assert_eq!(unportable(&schema), ["/properties/days"]);
}

#[test]
fn every_kind_that_is_not_a_string_counts_and_strings_do_not() {
    for kind in ["integer", "number", "boolean"] {
        let schema = json!({"type": kind, "enum": [1]});
        assert_eq!(unportable(&schema), [""], "{kind}");
    }
    assert_eq!(
        unportable(&json!({"enum": ["a", 2]})),
        [""],
        "没写类型、值不全是字符串"
    );
    assert!(unportable(&json!({"enum": ["a", "b"]})).is_empty());
    assert!(unportable(&json!({"type": "string", "enum": ["a"]})).is_empty());
}

#[test]
fn only_schema_positions_are_walked() {
    let schema = json!({
        "type": "object",
        "properties": {
            "enum": {"type": "string"},
            "list": {"type": "array", "items": {"type": "number", "enum": [0.5]}},
            "either": {"anyOf": [{"type": "string"}, {"type": "boolean", "enum": [true]}]},
            "a/b": {"type": "integer", "enum": [1]},
            "map": {"type": "object", "additionalProperties": {"type": "integer", "enum": [2]}}
        },
        "description": {"enum": [1]}
    });
    let mut found = unportable(&schema);
    found.sort();
    assert_eq!(
        found,
        [
            "/properties/a~1b",
            "/properties/either/anyOf/1",
            "/properties/list/items",
            "/properties/map/additionalProperties",
        ],
        "叫 enum 的属性名、description 底下的不当关键字"
    );
}

/// 出厂的工具说明（`resources/` 下每一份 `tools/<名字>.json`）没有不是各家都收的写法：核心自己写错了照样红。
#[test]
fn shipped_tool_descriptions_are_portable() {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let found = scan(&resources);
    assert_eq!(found, Vec::<Found>::new());
    let checked = walkdir(&resources);
    assert!(checked >= 30, "查到的出厂工具说明太少：{checked}");
}

/// `resources/` 下有几份 `tools/<名字>.json`（证明上一条真的查到了东西）。
fn walkdir(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                walkdir(&path)
            } else {
                usize::from(super::is_spec(&path))
            }
        })
        .sum()
}

#[test]
fn a_package_folder_is_scanned_for_tool_descriptions() {
    let dir = std::env::temp_dir().join(format!("miyu-portable-{}", std::process::id()));
    let put = |path: &str, text: &str| {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
        std::fs::write(path, text).expect("写得进");
    };
    put(
        "data/qq/tools/ranking.json",
        r#"{"description":"d","parameters":{"type":"object","properties":{"days":{"type":"integer","enum":[1,7]}}}}"#,
    );
    put(
        "data/qq/tools/fine.json",
        r#"{"description":"d","parameters":{"type":"object","properties":{"s":{"type":"string","enum":["a"]}}}}"#,
    );
    put("data/qq/tools/broken.json", "{");
    put(
        "notes/ranking.json",
        r#"{"parameters":{"type":"integer","enum":[1]}}"#,
    );
    let found = scan(&dir);
    let _left_in_temp = std::fs::remove_dir_all(&dir);
    assert_eq!(
        found,
        [Found {
            file: "data/qq/tools/ranking.json".to_string(),
            at: vec!["/properties/days".to_string()],
        }],
        "只认 tools/ 底下的；写坏的跳过"
    );
}

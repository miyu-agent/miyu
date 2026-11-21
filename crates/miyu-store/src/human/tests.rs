//! 出厂的字表没有这一版不认识的格（施工 FX-7）：读的时候不认识的格不管（钉旧核心的扩展、头读新核心装的字表），所以出厂的
//! 写错了格（拼错了 `options`、加了没有的格）读的时候不报，拦在这里。办法是读一遍、照读到的样子写回 JSON，原文里写回去没有的
//! 格就是不认识的。

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::File;

/// 原文 `text` 里这一版不认识的格，一处一个路径（例如 `/config/items/log.level/optoins`）。
fn unknown(text: &str) -> Vec<String> {
    let written: Value = serde_json::from_str(text).expect("是 JSON");
    let read: File = serde_json::from_str(text).expect("读得成");
    let read = serde_json::to_value(read).expect("写得回去");
    let mut found = Vec::new();
    missing(&written, &read, "", &mut found);
    found
}

/// `written` 里有、`read` 里没有的格，路径记进 `found`。
fn missing(written: &Value, read: &Value, at: &str, found: &mut Vec<String>) {
    match (written, read) {
        (Value::Object(written), Value::Object(read)) => {
            for (key, inner) in written {
                let path = format!("{at}/{key}");
                match read.get(key) {
                    Some(back) => missing(inner, back, &path, found),
                    None => found.push(path),
                }
            }
        }
        (Value::Array(written), Value::Array(read)) => {
            for (n, (inner, back)) in written.iter().zip(read).enumerate() {
                missing(inner, back, &format!("{at}/{n}"), found);
            }
        }
        _ => {}
    }
}

/// 源码树里有字表的几处：内核，`software/` 下的每个包，包文件夹自己带着字的（`packages/<编号>/data/`）。
fn places() -> Vec<PathBuf> {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut places = vec![resources.join("core")];
    for (dir, inner) in [("software", ""), ("packages", "data")] {
        let entries = std::fs::read_dir(resources.join(dir)).expect("有这个目录");
        places.extend(
            entries
                .map(|entry| entry.expect("读得了").path().join(inner))
                .filter(|path| path.join("human").is_dir()),
        );
    }
    places
}

#[test]
fn shipped_words_have_no_fields_this_version_does_not_know() {
    let mut checked = 0;
    let mut problems = Vec::new();
    for place in places() {
        let human = place.join("human");
        for entry in std::fs::read_dir(&human).expect("读得了") {
            let file = entry.expect("读得了").path();
            if file.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let text = std::fs::read_to_string(&file).expect("读得了");
            checked += 1;
            problems.extend(
                unknown(&text)
                    .into_iter()
                    .map(|path| format!("{}: {path}", file.display())),
            );
        }
    }
    assert!(
        checked >= 6,
        "至少内核和一个包各三种语言，只查到 {checked} 份"
    );
    assert_eq!(problems, Vec::<String>::new());
}

/// 上面那条拦得住东西：每一层拼错、多出来的格都报出路径，`block` 写了不认识的一种也报，认识的格（包括可以不写的）不报。
#[test]
fn a_misspelled_field_is_found_at_every_level() {
    let text = r#"{
        "tools": {"read": {"name": "读取", "subject": "file_path", "icon": "→", "block": "command", "colour": "red"},
                  "edit": {"name": "编辑", "block": "diff"}},
        "said": {"a": "A"},
        "config": {
            "items": {"log.level": {"name": "级别", "description": "", "optoins": {},
                                    "option_notices": {"info": "提示"},
                                    "fields": {"x": {"name": "X", "description": "", "sugestions": []}}}},
            "pages": {},
            "group": {}
        },
        "extra": 1
    }"#;
    let mut found = unknown(text);
    found.sort();
    assert_eq!(
        found,
        [
            "/config/group",
            "/config/items/log.level/fields/x/sugestions",
            "/config/items/log.level/optoins",
            "/extra",
            "/tools/edit/block",
            "/tools/read/colour",
        ]
    );
}

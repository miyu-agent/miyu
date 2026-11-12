//! `miyu bus list` 一行的样子（施工 BUS-3）：名字、包照最长的对齐，种类、可不可用照语言；一个都没有的是一句。

use serde_json::json;

use super::lines;
use crate::language::Language;

#[test]
fn a_name_is_one_aligned_line() {
    let listed = json!({"names": [
        {"name": "stt", "kind": "provides", "package": "voice-input", "ready": true},
        {"name": "tts.engine", "kind": "plugs", "package": "gptsovits", "ready": false},
    ]});
    assert_eq!(
        lines(&listed, Language::Chinese),
        [
            "stt         提供  voice-input  可用",
            "tts.engine  接进  gptsovits    不可用",
        ]
    );
    assert_eq!(
        lines(&listed, Language::English)[1],
        "tts.engine  plugs  gptsovits    not ready"
    );
    assert_eq!(
        lines(&json!({"names": []}), Language::Chinese),
        ["总线上还没有名字"]
    );
}

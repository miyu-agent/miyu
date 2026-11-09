//! `[export]` 的总线名字（施工 BUS-1，设计 67 B4、B6、B10）。

use crate::package::{Code, read, under};

/// 一个扩展包，`export` 接在最后。
fn extension(export: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Voice\" }}\n\n[command]\nname = \"voice\"\nprogram = \"miyu-voice\"\nabout = {{ en = \"V\" }}\n\n[process]\nstart = \"manual\"\n\n[export]\n{export}\n"
    )
}

#[test]
fn provides_and_plugs_are_read_with_their_methods() {
    let manifest = read(&extension(
        "provides = [\"stt\"]\nplugs = [\"tts.engine\"]\nmethods = [\"stt.transcribe\", \"stt.stop\", \"tts.engine.synthesize\", \"own\"]",
    ))
    .unwrap();
    assert_eq!(manifest.export.provides, ["stt"]);
    assert_eq!(manifest.export.plugs, ["tts.engine"]);
    assert!(manifest.export.exports("own"), "不在名字底下的方法照收");
}

#[test]
fn bad_names_and_names_without_methods_are_refused() {
    for (export, why) in [
        ("provides = [\"STT\"]\nmethods = [\"STT.x\"]", "大写"),
        ("provides = [\"stt.\"]\nmethods = [\"stt.x\"]", "空的一段"),
        ("provides = [\"1stt\"]\nmethods = [\"1stt.x\"]", "数字开头"),
        (
            "provides = [\"stt\", \"stt\"]\nmethods = [\"stt.x\"]",
            "写了两遍",
        ),
        (
            "provides = [\"stt\"]\nplugs = [\"stt\"]\nmethods = [\"stt.x\"]",
            "两样都写",
        ),
        (
            "provides = [\"stt\"]\nmethods = [\"transcribe\"]",
            "名字底下没有方法",
        ),
        (
            "provides = [\"stt\"]\nmethods = [\"stt.engine.transcribe\"]",
            "短名带点不算 stt 底下",
        ),
    ] {
        let problem = read(&extension(export)).unwrap_err();
        assert_eq!(problem.code, Code::BadExport, "{why}");
        assert!(problem.line.is_some(), "{why}");
    }
}

#[test]
fn a_short_name_is_one_part() {
    assert!(under("stt.transcribe", "stt"));
    assert!(under("stt.engine.transcribe", "stt.engine"));
    assert!(
        !under("stt.engine.transcribe", "stt"),
        "B10：只归最近的名字"
    );
    assert!(!under("stt.", "stt"));
    assert!(!under("sttx.transcribe", "stt"));
}

//! `[export]` 的总线名字（施工 BUS-1，设计 67 B4、B6、B10）；方法都要在声明的名字底下（施工 BUS-5，B5）。

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
        "provides = [\"stt\"]\nplugs = [\"tts.engine\"]\nmethods = [\"stt.transcribe\", \"stt.stop\", \"tts.engine.synthesize\"]",
    ))
    .unwrap();
    assert_eq!(manifest.export.provides, ["stt"]);
    assert_eq!(manifest.export.plugs, ["tts.engine"]);
    assert_eq!(
        manifest.export.methods,
        ["stt.transcribe", "stt.stop", "tts.engine.synthesize"],
        "全在名字底下的读得成"
    );
}

/// 施工 BUS-5（设计 67 B5）：`methods` 里每一个都要在这个包自己 `provides`、`plugs` 的某个名字底下。不在的谁也调不到（扩展
/// 照包编号调不了别的包，总线只转名字底下的），清单读不成。
#[test]
fn methods_outside_every_declared_name_are_refused() {
    for (export, detail, why) in [
        (
            "provides = [\"stt\"]\nmethods = [\"stt.transcribe\", \"own\"]",
            "own",
            "一段的、不带名字的",
        ),
        (
            "provides = [\"stt\"]\nplugs = [\"tts.engine\"]\nmethods = [\"stt.transcribe\", \"tts.engine.synthesize\", \"tts.play\"]",
            "tts.play",
            "带了前缀，可那个名字没声明",
        ),
        (
            "provides = [\"stt\"]\nmethods = [\"stt.transcribe\", \"stt.engine.transcribe\"]",
            "stt.engine.transcribe",
            "短名带点的不算 stt 底下（B10）",
        ),
        ("methods = [\"add\"]", "add", "没声明名字却写了方法"),
        (
            "methods = [\"chat.media\", \"chat.venue\"]",
            "chat.media",
            "没声明名字，带点也不算",
        ),
    ] {
        let problem = read(&extension(export)).unwrap_err();
        assert_eq!(problem.code, Code::BadExport, "{why}");
        assert_eq!(problem.detail, detail, "{why}：说的是哪一个方法");
        assert!(problem.line.is_some(), "{why}");
    }
    assert!(
        read(&extension("")).unwrap().export.is_empty(),
        "空的 [export] 照旧读得成"
    );
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

//! 装好以后问不问、答什么算打开（作者说明第九节第 1 条）。

use serde_json::json;

use super::*;

#[test]
fn a_switchable_package_that_is_off_or_waits_for_approval_is_offered() {
    let none = json!({});
    let needs = json!({"capabilities": [{"id": "tools", "name": "工具"}]});
    assert!(wanted(&json!({"enabled": false, "status": "off"}), &none));
    assert!(
        wanted(&json!({"enabled": true, "status": "stopped"}), &needs),
        "开着却没在跑、要能力：能力还没批"
    );
    assert!(
        !wanted(&json!({"enabled": true, "status": "stopped"}), &none),
        "不要能力的停下了是别的原因"
    );
    assert!(!wanted(
        &json!({"enabled": true, "status": "running"}),
        &needs
    ));
    assert!(!wanted(
        &json!({"enabled": true, "status": "starting"}),
        &needs
    ));
    assert!(
        !wanted(
            &json!({"enabled": false, "status": "program_missing"}),
            &none
        ),
        "程序不在的打不开"
    );
    assert!(
        !wanted(&json!({"status": "ready"}), &needs),
        "没有开关的不问"
    );
}

#[test]
fn the_capabilities_come_from_the_preview() {
    let preview = json!({"capabilities": [{"id": "tools", "name": "工具"}, {"id": "network", "name": "联网"}]});
    assert_eq!(capabilities(&preview), ["tools", "network"]);
    assert!(capabilities(&json!({})).is_empty());
}

#[test]
fn an_empty_or_yes_answer_opens_and_anything_else_does_not() {
    let language = Language::English;
    for (typed, opens) in [
        ("\n", true),
        ("y\n", true),
        ("YES\n", true),
        ("n\n", false),
        ("later\n", false),
        ("", false),
    ] {
        let mut out = Vec::new();
        let got = agreed(&mut typed.as_bytes(), &mut out, (false, "app"), &language);
        assert_eq!(got, opens, "{typed:?}");
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("Enable 'app' now? [Y/n] ")
        );
    }
    let mut out = Vec::new();
    assert!(
        agreed(&mut "n\n".as_bytes(), &mut out, (true, "app"), &language),
        "--yes 不问"
    );
    assert!(out.is_empty());
}

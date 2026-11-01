//! 选 Miyu 自己的东西（施工 PC-3，设计 53 第四节）：`type = "model"`、`"persona"`、`"knowledge"`。

use super::*;

#[test]
fn model_persona_and_knowledge_read() {
    let x = |kind: &str| {
        setting(&format!(
            "[settings.x]\ntype = \"{kind}\"\nname = {{ en = \"X\" }}\n"
        ))
    };
    assert_eq!(x("model").kind, SettingKind::Model);
    assert_eq!(x("persona").kind, SettingKind::Persona);
    assert_eq!(x("knowledge").kind, SettingKind::Knowledge);
    let all = [x("model"), x("persona"), x("knowledge")];
    let shapes: Vec<(Kind, Control)> = items("web", "web", &all)
        .iter()
        .map(|item| (item.kind, item.ui.control))
        .collect();
    assert_eq!(
        shapes,
        [
            (Kind::Reference, Control::Text),
            (Kind::Persona, Control::Text),
            (Kind::Knowledge, Control::Text),
        ],
        "模型是配置清单的「引用」（模型、池都行）"
    );
    let defaulted = setting(
        "[settings.x]\ntype = \"model\"\ndefault = \"deepseek/deepseek-chat\"\nname = { en = \"X\" }\n",
    );
    assert_eq!(
        defaulted.default,
        Some(Value::Text("deepseek/deepseek-chat".into()))
    );
    let pooled =
        setting("[settings.x]\ntype = \"model\"\ndefault = \"@fast\"\nname = { en = \"X\" }\n");
    assert_eq!(pooled.default, Some(Value::Text("@fast".into())), "池也收");
    let listed = setting(
        "[settings.x]\ntype = \"list\"\nelement = \"persona\"\ndefault = [\"cat\"]\nname = { en = \"X\" }\n",
    );
    assert_eq!(
        listed.kind,
        SettingKind::List(Box::new(SettingKind::Persona))
    );
    for wrong in [
        "type = \"model\"\ndefault = \"no-slash\"",
        "type = \"persona\"\ndefault = \"Cat\"",
        "type = \"knowledge\"\ndefault = \"a/b\"",
    ] {
        assert_eq!(
            super::wrong(&format!("[settings.x]\n{wrong}\nname = {{ en = \"X\" }}\n")),
            Code::BadDefault,
            "{wrong}"
        );
    }
}

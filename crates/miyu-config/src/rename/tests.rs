//! 供应商改名（施工 8-34）：表头、子表、点号连着的键、行内表都跟着换名，别处的字一字不差；引用、模型的列表里指着旧编号
//! 的换成新的，前缀像的、池、别家的不动。

use super::*;
use crate::item::Layer;
use crate::parse::parse;

crate::settings! {
    /// 测试用的用途：一个引用。
    pub struct Uses in "uses" {
        /// 主对话的。
        chat: Option<String> = none {
            kind: reference,
            layers: [System],
            applies: new_session,
            ui: { page: "models", group: "uses", control: text },
        },
        /// 另一个引用：指着池。
        side: Option<String> = none {
            kind: reference,
            layers: [System],
            applies: new_session,
            ui: { page: "models", group: "uses", control: text },
        },
    }
}

crate::settings! {
    /// 测试用的池：成员是模型的列表。
    pub struct Pools in "pools.<id>" {
        /// 成员。
        models: Option<Vec<String>> = none {
            kind: models,
            layers: [System],
            applies: next_turn,
            ui: { page: "models", group: "pools", control: list },
        },
    }
}

#[test]
fn the_table_and_its_subtables_are_renamed_and_the_rest_is_kept() {
    let text = "#:schema x\n# 头上的注释\n[ui]\nlanguage = \"zh\"  # 行尾注释\n\n[providers.old]\ndriver = \"openai-chat\"\nkey = { secret = \"k\" }\n\n[providers.old.compat]\nx = 1\n\n[providers.other]\ndriver = \"x\"\n\n[providers.old.models.\"a/b\"]\nwindow = 8\n";
    let renamed = rename_table(text, "providers", "old", "new")
        .expect("放得进")
        .expect("有这张表");
    assert!(
        renamed.starts_with("#:schema x\n# 头上的注释\n[ui]\nlanguage = \"zh\"  # 行尾注释\n"),
        "{renamed}"
    );
    for header in [
        "[providers.new]",
        "[providers.new.compat]",
        "[providers.new.models.\"a/b\"]",
        "[providers.other]",
    ] {
        assert!(renamed.contains(header), "{header}：{renamed}");
    }
    assert!(!renamed.contains("providers.old"), "{renamed}");
    assert!(
        renamed.contains("key = { secret = \"k\" }"),
        "密钥跟着表走：{renamed}"
    );
    // 点号连着的键、行内表
    let dotted = rename_table("providers.old.driver = \"x\"\n", "providers", "old", "new")
        .unwrap()
        .unwrap();
    assert!(dotted.contains("providers.new.driver = \"x\""), "{dotted}");
    let inline = rename_table(
        "providers = { old = { driver = \"x\" } }\n",
        "providers",
        "old",
        "new",
    )
    .unwrap()
    .unwrap();
    assert!(inline.contains("new = { driver = \"x\" }"), "{inline}");
}

#[test]
fn nothing_to_rename_or_a_taken_name() {
    assert_eq!(
        rename_table("[ui]\nx = 1\n", "providers", "old", "new"),
        Ok(None)
    );
    assert_eq!(
        rename_table("[providers.a]\nx = 1\n", "providers", "old", "new"),
        Ok(None)
    );
    assert!(
        rename_table(
            "[providers.old]\n[providers.new]\n",
            "providers",
            "old",
            "new"
        )
        .is_err()
    );
    assert!(rename_table("providers = 1\n", "providers", "old", "new").is_err());
    assert!(rename_table("[[broken\n", "providers", "old", "new").is_err());
}

#[test]
fn references_to_the_old_name_are_retargeted() {
    let items = [Uses::ITEMS, Pools::ITEMS].concat();
    let text = "[uses]\nchat = \"old/m\"\nside = \"@free\"\n\n[pools.free]\nmodels = [\"old/x\", \"oldish/y\", \"b/z\", \"old/a/b\"]\n\n[pools.paid]\nmodels = [\"b/z\"]\n";
    let parsed = parse(&items, Layer::System, text).expect("写法对");
    let mut found = retargeted(&items, &parsed, "old", "new");
    found.sort_by(|a, b| a.0.cmp(&b.0));
    let text = |value: &str| Value::Text(value.to_string().into());
    assert_eq!(
        found,
        vec![
            (
                "pools.free.models".to_string(),
                Value::List(
                    vec![
                        text("new/x"),
                        text("oldish/y"),
                        text("b/z"),
                        text("new/a/b")
                    ]
                    .into()
                )
            ),
            ("uses.chat".to_string(), text("new/m")),
        ]
    );
}

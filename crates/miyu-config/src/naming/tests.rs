//! 命名的规矩（施工 UX-1）：反着说的开关键、句子样的名字、反着说的开关名字各拦一处；软件术语的名字、不是开关的「不」照收。

use super::*;
use crate::item::{Applies, Control, Layer, Ui};
use crate::words::ItemWords;

fn item(key: &'static str, kind: Kind) -> Item {
    Item {
        key,
        kind,
        default: None,
        layers: &[Layer::System],
        tighten: None,
        env: None,
        applies: Applies::Now,
        ui: Ui {
            page: "general",
            group: "core",
            package: None,
            common: false,
            control: Control::Toggle,
            hidden: false,
        },
    }
}

fn words(names: &[(&str, &str)]) -> ConfigWords {
    let mut words = ConfigWords::default();
    for (key, name) in names {
        words.items.insert(
            (*key).to_string(),
            ItemWords {
                name: (*name).to_string(),
                description: String::new(),
                options: Default::default(),
                option_descriptions: Default::default(),
                option_notices: Default::default(),
                fields: Default::default(),
                suggestions: Vec::new(),
            },
        );
    }
    words
}

#[test]
fn a_switch_key_says_what_true_turns_on() {
    let items = [
        item("packages.kit.off", Kind::Bool),
        item("net.no_proxy", Kind::Bool),
        item("skills.external.enabled", Kind::Bool),
        item("log.level_off", Kind::Text { max: 8 }),
    ];
    let problems = check(&items, &ConfigWords::default(), "zh");
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems[0].starts_with("packages.kit.off"), "{problems:?}");
    assert!(problems[1].starts_with("net.no_proxy"), "{problems:?}");
}

#[test]
fn names_are_terms_not_sentences() {
    let items = [
        item("a.on", Kind::Bool),
        item("b.hold", Kind::List(&Kind::Name)),
    ];
    let zh = words(&[
        ("a.on", "把这个包关掉"),
        ("b.hold", "不升级的包"),
        ("c.name", "称呼"),
        ("d.name", "AI 怎么称呼你"),
    ]);
    let problems = check(&items, &zh, "zh");
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert!(
        problems.iter().any(|one| one.contains("反着说")),
        "开关带「关掉」"
    );
    assert!(
        !problems.iter().any(|one| one.starts_with("b.hold")),
        "不是开关的「不」照收"
    );
    assert!(!problems.iter().any(|one| one.starts_with("c.name")));
    let en = words(&[
        ("a.on", "Turn this package off for me"),
        ("c.name", "Preferred name"),
        ("d.name", "Remind the AI"),
        ("e.name", "Mail domains"),
    ]);
    let problems = check(&items, &en, "en");
    assert_eq!(
        problems.len(),
        3,
        "整词比：domains 里的 ai 不算：{problems:?}"
    );
}

//! 留下的设置项（施工 PK-5补）：第一段是留下的前缀的 `unknown_key` 拿掉、记是谁的；别的不认识的、写错类型的照报。

use miyu_config::Layer;
use miyu_config::parse::parse;
use miyu_config::problem::Code;

use super::*;

fn left() -> Left {
    Left::new([
        ("xcfg".to_string(), "xcfg".to_string()),
        ("kt".to_string(), "kit".to_string()),
    ])
}

#[test]
fn leftover_keys_are_taken_out_and_counted() {
    let items = crate::settings::UiSettings::ITEMS.to_vec();
    let text =
        "ui.language = 3\nxcfg.port = 9000\nuii.language = \"en\"\n\n[kt]\na = 1\nb.c = \"x\"\n";
    let mut parsed = parse(&items, Layer::System, text).expect("写法对");
    let taken = left().take(&mut parsed);
    assert_eq!(
        taken,
        vec![
            ("xcfg.port".to_string(), "xcfg".to_string()),
            ("kt.a".to_string(), "kit".to_string()),
            ("kt.b.c".to_string(), "kit".to_string()),
        ]
    );
    let codes: Vec<(Code, Option<&str>)> = parsed
        .problems
        .iter()
        .map(|problem| (problem.code, problem.key.as_deref()))
        .collect();
    assert_eq!(
        codes,
        vec![
            (Code::WrongType, Some("ui.language")),
            (Code::UnknownKey, Some("uii.language")),
        ],
        "别的照报"
    );
    let counts = counted(&taken);
    assert_eq!(counts.get("kit"), Some(&2));
    assert_eq!(counts.get("xcfg"), Some(&1));
}

#[test]
fn nothing_left_takes_nothing() {
    let items = crate::settings::UiSettings::ITEMS.to_vec();
    let mut parsed = parse(&items, Layer::System, "xcfg.port = 1\n").expect("写法对");
    assert!(Left::default().take(&mut parsed).is_empty());
    assert_eq!(parsed.problems.len(), 1);
    assert_eq!(left().owner("kt"), Some("kit"));
    assert_eq!(left().owner("kit.a"), None, "照前缀认，不照编号");
}

#[test]
fn the_first_segment_decides_even_when_quoted() {
    assert_eq!(left().owner("\"xcfg\".port"), Some("xcfg"));
    assert_eq!(left().owner("ui.xcfg"), None);
}

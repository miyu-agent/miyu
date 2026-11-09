//! 换了预设那一块（施工 DM-8，`facts/preset.rs`）：模板造的时候就查；写换成的和换之前的，空的写 `none`；以前的快照没有这份
//! 模板的、安静的没有这一块。

use super::*;

fn templates(preset: Option<&str>) -> Result<FactTemplates, TemplateError> {
    FactTemplates::new(
        r#"<e t="{time}" z="{timezone}" d="{cwd}"/>"#,
        r#"<p l="{level}"/>"#,
        "<cut/>",
        None,
        None,
    )
    .unwrap()
    .with_preset_changed(preset)
}

#[test]
fn it_says_what_it_was_switched_to_and_from() {
    let facts = templates(Some(r#"<s i="{id}" p="{previous}"/>"#)).unwrap();
    let fact = facts.preset_changed("minimal", "dev").unwrap();
    assert_eq!(fact.kind.as_str(), PRESET);
    assert_eq!(fact.text, r#"<s i="minimal" p="dev"/>"#);
    assert!(!fact.after);
    assert_eq!(
        facts.preset_changed("", "dev").unwrap().text,
        r#"<s i="none" p="dev"/>"#,
        "不用预设写 none"
    );
    assert_eq!(
        facts.preset_changed("dev", "").unwrap().text,
        r#"<s i="dev" p="none"/>"#
    );
}

#[test]
fn an_older_snapshot_or_a_quiet_session_has_none() {
    assert_eq!(templates(None).unwrap().preset_changed("dev", ""), None);
    let quiet = templates(Some(r#"<s i="{id}"/>"#)).unwrap().quiet();
    assert_eq!(quiet.preset_changed("dev", ""), None);
}

#[test]
fn a_broken_template_is_refused_when_made() {
    assert!(
        templates(Some("<s i=\"{nope}\"/>")).is_err(),
        "要了没有的字段"
    );
    assert!(templates(Some("<s i=\"{id\"/>")).is_err(), "写法坏了");
}

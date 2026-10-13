//! 「关于你」那一块（施工 P-9）：两项都有、只有一项、都空、截断、称呼里的换行，接在 system 的哪里。

use super::*;
use crate::test_support::engineer;

const TEMPLATE: &str = "<user>\nname: {name}\nabout: {about}\n</user>\n";

fn profile(name: &str, about: &str) -> Profile {
    Profile {
        name: name.to_string(),
        about: about.to_string(),
    }
}

#[test]
fn both_fields_make_a_block_and_empty_ones_drop_their_line() {
    assert_eq!(
        profile(" 小林 ", "写 Rust 的\n独立开发者\n")
            .block(TEMPLATE)
            .as_deref(),
        Some("<user>\nname: 小林\nabout: 写 Rust 的\n独立开发者\n</user>")
    );
    assert_eq!(
        profile("小林", "  ").block(TEMPLATE).as_deref(),
        Some("<user>\nname: 小林\n</user>")
    );
    assert_eq!(
        profile("", "喜欢先看结论").block(TEMPLATE).as_deref(),
        Some("<user>\nabout: 喜欢先看结论\n</user>")
    );
    assert_eq!(profile(" ", "\n").block(TEMPLATE), None, "都空的没有");
}

#[test]
fn the_name_is_one_line_and_both_are_clipped() {
    assert_eq!(
        profile("小\n林", "").block(TEMPLATE).as_deref(),
        Some("<user>\nname: 小 林\n</user>")
    );
    let long = "名".repeat(NAME_MAX + 5);
    let block = profile(&long, &"字".repeat(ABOUT_MAX + 5))
        .block(TEMPLATE)
        .expect("有");
    assert!(block.contains(&format!("name: {}\n", "名".repeat(NAME_MAX))));
    assert!(block.contains(&format!("about: {}\n", "字".repeat(ABOUT_MAX))));
}

#[test]
fn the_block_follows_the_persona_and_is_kept_in_the_snapshot() {
    let snapshot = engineer().with_venue("场所");
    let with = snapshot
        .clone()
        .with_profile(Some("<user>\nname: 小林\n</user>".to_string()));
    assert_eq!(
        with.system,
        format!("{}\n\n<user>\nname: 小林\n</user>", snapshot.system),
        "接在人设、场所说明后面，空一行"
    );
    assert!(snapshot.system.ends_with("场所"));
    assert_eq!(with.profile.as_deref(), Some("<user>\nname: 小林\n</user>"));
    let without = snapshot.clone().with_profile(None);
    assert_eq!(without, snapshot, "没有的一字不变");
}

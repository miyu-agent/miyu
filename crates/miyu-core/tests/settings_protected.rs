//! 设置页「受保护的命令」在核心登记的清单里的位置（施工 HZ-1，设计 `11-权限与沙盒.md` 的 A18，`docs/blueprint/config.md`）。

use std::path::Path;

use miyu_core::settings::items;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

/// 出厂带的语言。
const LANGUAGES: [&str; 3] = ["zh", "en", "ja"];

/// 「受保护的命令」排在「隐私与安全」那一页最上面（施工 HZ-1，2026-11-11 项目主人定）：这一页先出现的项就是它的，私密目录
/// 在后面；五个开关挨着。组带说明（哪几类永远拒、不能关写在里面），三种语言都有；别的组没写说明。
#[test]
fn the_protected_commands_lead_the_privacy_page_and_carry_a_description() {
    let listed = items();
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut groups: Vec<&str> = Vec::new();
    for item in listed.iter().filter(|item| item.ui.page == "privacy") {
        if !groups.contains(&item.ui.group) {
            groups.push(item.ui.group);
        }
    }
    assert_eq!(groups.first(), Some(&"protected"), "{groups:?}");
    let at = |group: &str| groups.iter().position(|one| *one == group);
    assert!(at("protected") < at("private"), "{groups:?}");
    let keys: Vec<&str> = listed
        .iter()
        .filter(|item| item.ui.group == "protected")
        .map(|item| item.key)
        .collect();
    assert_eq!(
        keys,
        [
            "permission.confirm.git",
            "permission.confirm.delete",
            "permission.confirm.power",
            "permission.confirm.pipe_to_shell",
            "permission.confirm.container",
        ]
    );
    for language in LANGUAGES {
        let words = Human::load(&ResourceRoot::at(resources.clone()), language)
            .unwrap_or_else(|error| panic!("{language} 的字读得出来：{error}"));
        let described = words.group_description("protected").unwrap_or_default();
        assert!(!described.trim().is_empty(), "{language}");
        for item in &listed {
            if item.ui.group != "protected" {
                assert_eq!(
                    words.group_description(item.ui.group),
                    None,
                    "{language} {}",
                    item.ui.group
                );
            }
        }
    }
}

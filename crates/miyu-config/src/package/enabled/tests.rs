//! 包的「启用」（施工 UX-1）：每个包一项、挂在「软件包」页这个包那一组、当场生效；同一个编号拼两次是同一份；认得出真的键是
//! 哪个包的，别的键不认。

use super::*;

#[test]
fn each_package_has_one_switch_in_its_group() {
    let kit = item_for("kit");
    assert_eq!(kit.key, "packages.kit.enabled");
    assert_eq!(kit.kind, Kind::Bool);
    assert_eq!(
        (kit.ui.page, kit.ui.group, kit.ui.package, kit.ui.hidden),
        ("packages", "kit", Some("kit"), false)
    );
    assert_eq!(kit.applies, Applies::Now, "开关就是起停进程，当场生效");
    assert!(std::ptr::eq(kit, item_for("kit")), "同一个编号只拼一次");
}

#[test]
fn the_package_is_read_from_the_key() {
    assert_eq!(id_of("packages.kit.enabled").as_deref(), Some("kit"));
    for other in [
        "packages.kit.off",
        "packages.enabled",
        "packages.sources.kit.url",
        "features.kit",
    ] {
        assert_eq!(id_of(other), None, "{other}");
    }
}

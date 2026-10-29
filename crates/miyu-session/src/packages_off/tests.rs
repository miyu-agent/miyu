//! 关掉某个包的那一项（施工 U23 末上）：只认写成开着的；每个包那一项挂在「软件包」页这个包那一组；带占位的
//! 那一项不上设置页；具体的键先认具体的那一项。

use super::*;

#[test]
fn only_packages_switched_off_are_read() {
    let mut values = Values::defaults(&[]);
    values.set("packages.kit.off", Value::Bool(true));
    values.set("packages.relay.off", Value::Bool(false));
    values.set("features.skill-weather", Value::Bool(true));
    assert_eq!(of(&values).into_iter().collect::<Vec<_>>(), ["kit"]);
}

#[test]
fn each_package_has_its_own_switch_in_its_group() {
    let kit = item_for("kit");
    assert_eq!(kit.key, "packages.kit.off");
    assert_eq!(
        (kit.ui.page, kit.ui.group, kit.ui.package, kit.ui.hidden),
        ("packages", "kit", Some("kit"), false)
    );
    assert_eq!(kit.layers, [Layer::System], "只写个人层");
    assert!(ITEMS[0].ui.hidden, "带占位的不上设置页");
    let items = [ITEMS[0].clone(), kit];
    assert_eq!(
        key::item_of(&items, "packages.kit.off").map(|item| item.ui.group),
        Some("kit"),
        "具体的键先认具体的那一项"
    );
    assert_eq!(
        key::item_of(&items, "packages.later.off").map(|item| item.key),
        Some(OFF),
        "没装的编号认带占位的那一项"
    );
}

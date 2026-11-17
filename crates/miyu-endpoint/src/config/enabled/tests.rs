//! 包的「启用」在协议上（施工 UX-1）：请求里的键照有没有开关分开；没写 `keys` 的是全部「启用」；没有开关的包、别的键照常认。

use super::*;

fn switches() -> Vec<(String, bool)> {
    vec![("kit".to_string(), false), ("relay".to_string(), true)]
}

#[test]
fn keys_are_split_by_whether_the_package_has_a_switch() {
    let asked = [
        "packages.kit.enabled".to_string(),
        "packages.kit.enabled".to_string(),
        "packages.ui-only.enabled".to_string(),
        "ui.language".to_string(),
        "packages.relay.off".to_string(),
    ];
    let (mine, rest) = split(&switches(), Some(&asked));
    assert_eq!(mine, [("kit".to_string(), false)], "同一个键只算一次");
    assert_eq!(
        rest,
        Some(vec![
            "packages.ui-only.enabled".to_string(),
            "ui.language".to_string(),
            "packages.relay.off".to_string(),
        ]),
        "没有开关的包、删掉的 off 照常认（认不出的报不认识）"
    );
    let (all, rest) = split(&switches(), None);
    assert_eq!(all, switches());
    assert_eq!(rest, None);
}

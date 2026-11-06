//! 扩展提供的驱动（施工 MD-1，设计 66 第四节第 2 条）：只认手写的那一格。从 `tests.rs` 分出来，照它的几样（`texts`、`wire`）。

use serde_json::json;

use super::super::*;
use super::{texts, wire};
use crate::test_support::Held;

/// 照配置的字 `source` 合出来的最终值，`providers.<id>.driver` 的选项接上装着的 `pi/ag`（施工 MD-1：同核心起来时的做法）。
fn values_with_installed(source: &str) -> Values {
    use miyu_config::Layer;
    use miyu_config::merge::{Layers, merge};
    use miyu_config::parse::parse;
    let items: Vec<miyu_config::Item> = crate::test_support::items()
        .iter()
        .map(|item| miyu_config::driver_ref::with_installed(item, &["pi/ag".to_string()]))
        .collect();
    let parsed = parse(&items, Layer::System, source).expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    merge(&items, &layers, &|_| None).values()
}

/// 扩展提供的驱动只认手写的那一格（施工 MD-1，设计 66 第四节第 2 条）：配置里写 `<包>/<驱动名>` 的是它，造不出核心里的驱动、
/// 没有开关，模型的包名压不过它；档案、npm 表写成这个样子的不认（那一家、那个模型用不了），推不出来的默认永远是内置的。
#[test]
fn only_a_hand_written_driver_points_to_an_extension() {
    let held = Held::new(
        json!({
            "npm": {"@ai-sdk/anthropic": "anthropic", "@sneaky/pkg": "pi/ag"},
            "providers": {"sneaky": {"driver": "pi/ag", "base_url": "https://s.invalid"}}
        }),
        false,
    );
    let values = values_with_installed(
        "[providers.mine]\ndriver = \"pi/ag\"\nbase_url = \"https://m.invalid\"\n\n\
         [providers.sneaky]\nlocal = false\n\n\
         [providers.plain]\nbase_url = \"https://p.invalid\"\n",
    );
    let mine = provider(&values, &held.knowledge(), "mine").expect("手写的认");
    let reference = DriverRef {
        package: "pi".to_string(),
        name: "ag".to_string(),
    };
    assert_eq!(mine.driver, Driver::Extension(reference.clone()));
    assert_eq!(mine.driver.as_str(), "pi/ag");
    assert_eq!(mine.driver.extension(), Some(&reference));
    assert!(mine.build(texts()).is_none(), "核心里造不出它");
    assert!(!mine.switchable());
    let npm = &held.profiles.npm;
    let speaking = mine
        .for_model("m", &wire(Some("@ai-sdk/anthropic"), None), npm)
        .expect("手写的不看模型的包名");
    assert_eq!(speaking.driver, Driver::Extension(reference));
    // 档案写成扩展的样子：不认，这一家用不了，不会把它当成扩展的驱动。
    assert!(provider(&values, &held.knowledge(), "sneaky").is_err());
    // 推不出来的照默认：内置的。
    let plain = provider(&values, &held.knowledge(), "plain").expect("有地址就能用");
    assert_eq!(plain.driver, Driver::OpenAiChat);
    // npm 表把模型的包名换成扩展的样子：这个模型用不了，不指向扩展。
    assert!(
        plain
            .for_model("m", &wire(Some("@sneaky/pkg"), None), npm)
            .is_err()
    );
    // 写法：只有 `<包>/<驱动名>` 是扩展的；内置名照旧；`builtin` 不认扩展的。
    assert_eq!(Driver::parse("Pi/ag"), None);
    assert_eq!(Driver::builtin("pi/ag"), None);
    assert_eq!(Driver::builtin("anthropic"), Some(Driver::Anthropic));
}

//! 两层都配了的供应商另合一份系统配置的原样（施工 PL-1）：自用那份照旧逐格盖，共用那份只照系统配置、密钥只从系统那一份取；
//! 只有一层写了的不另合。

use crate::item::{Item, Kind, Layer};
use crate::merge::{Layers, Origin, merge};
use crate::parse::{Parsed, parse};
use crate::secret::Reference;
use crate::test_support::item;
use crate::value::Value;

fn items() -> Vec<Item> {
    let both: &'static [Layer] = &[Layer::System, Layer::Personal];
    vec![
        Item {
            key: "providers.<id>.base_url",
            kind: Kind::Url,
            default: None,
            layers: both,
            ..item("providers.<id>.base_url", &[], "")
        },
        Item {
            key: "providers.<id>.key",
            kind: Kind::Secret,
            default: None,
            layers: both,
            ..item("providers.<id>.key", &[], "")
        },
    ]
}

fn read(layer: Layer, source: &str) -> Parsed {
    parse(&items(), layer, source).expect("写法对")
}

fn no_env(_: &str) -> Option<String> {
    None
}

const SYSTEM: &str = "[providers.deepseek]\nbase_url = \"https://shared.example.invalid/v1\"\nkey = { secret = \"deepseek\" }\n\n[providers.only]\nbase_url = \"https://only.example.invalid/v1\"\n";

#[test]
fn both_layers_get_a_mine_and_a_shared_copy() {
    let system = read(Layer::System, SYSTEM);
    let personal = read(
        Layer::Personal,
        "[providers.deepseek]\nkey = { secret = \"mine\" }\n",
    );
    let layers = Layers {
        system: Some(&system),
        personal: Some(&personal),
        project: None,
    };
    let resolved = merge(&items(), &layers, &no_env);
    let value = |key: &str| resolved.get(key).map(|(value, _)| value.clone());
    // 自用：自己写的 key，别的照系统配置。
    assert_eq!(
        value("providers.deepseek.key"),
        Some(Value::Secret(Reference::Secret("mine".to_string())))
    );
    assert_eq!(
        value("providers.deepseek.base_url"),
        Some(Value::Text("https://shared.example.invalid/v1".into()))
    );
    // 共用的原样：只照系统配置，密钥只从系统那一份取。
    assert_eq!(
        value("providers.\"deepseek@system\".key"),
        Some(Value::Secret(Reference::SystemSecret(
            "deepseek".to_string()
        )))
    );
    assert_eq!(
        value("providers.\"deepseek@system\".base_url"),
        Some(Value::Text("https://shared.example.invalid/v1".into()))
    );
    assert!(matches!(
        resolved.get("providers.\"deepseek@system\".base_url"),
        Some((
            _,
            Origin::File {
                layer: Layer::System,
                ..
            }
        ))
    ));
    // 只有系统配置写了的不另合。
    assert_eq!(value("providers.\"only@system\".base_url"), None);
    assert_eq!(
        crate::key::names(resolved.values().keys(), "providers.<id>", &[]),
        ["deepseek", "deepseek@system", "only"]
    );
}

#[test]
fn one_layer_alone_has_no_copy() {
    let system = read(Layer::System, SYSTEM);
    let layers = Layers {
        system: Some(&system),
        personal: None,
        project: None,
    };
    let resolved = merge(&items(), &layers, &no_env);
    assert!(
        resolved.values().keys().all(|key| !key.contains("@system")),
        "没有个人设置的不另合"
    );
}

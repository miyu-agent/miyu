//! 包这一层（施工 PV-4）：包带的供应商照配置清单读、名字照语言挑；人自己的同编号、两个包撞了、驱动指别的包、读不过的格各报一种；
//! 人盖在包带的那一家上的照合，包不带了那一段不算。

use miyu_config::Item;
use miyu_config::merge::{Layers, Origin, merge};
use miyu_config::package::ProviderDecl;
use miyu_config::parse::{Parsed, parse};
use miyu_config::{Layer, Value};
use miyu_models::settings::{ModelSettings, ProviderSettings};

use super::{Clash, Declared, layer};

fn items() -> Vec<Item> {
    [ProviderSettings::ITEMS, ModelSettings::ITEMS]
        .concat()
        .into_iter()
        .map(|item| miyu_config::driver_ref::with_installed(&item, &["gravity/agy".to_string()]))
        .collect()
}

/// 包 `package` 带的一家：照清单读出来。
fn declared(package: &str, manifest_tail: &str) -> Declared {
    let text = format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"P\" }}\n\n[command]\nname = \"p\"\nprogram = \"p\"\nabout = {{ en = \"P\" }}\n\n[process]\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"agy\"\n\n{manifest_tail}"
    );
    let manifest = miyu_config::package::read(&text).expect("读得成");
    let provider: ProviderDecl = manifest.providers[0].clone();
    (package.to_string(), provider)
}

const AG: &str = "[providers.ag]\nname = { en = \"Antigravity\", zh = \"反重力\" }\ndriver = \"gravity/agy\"\nlocal = true\n\n[providers.ag.models.\"g3\"]\nwindow = 1000\n";

fn system(text: &str) -> Parsed {
    parse(&items(), Layer::System, text).expect("读得成")
}

fn resolved(
    packaged: &miyu_config::merge::Packaged,
    system: &Parsed,
) -> miyu_config::merge::Resolved {
    let layers = Layers {
        package: Some(packaged),
        system: Some(system),
        project: None,
    };
    merge(&items(), &layers, &|_| None)
}

#[test]
fn a_package_provider_lands_in_the_package_layer() {
    let none = system("");
    let (packaged, clashes) = layer(&items(), &[declared("gravity", AG)], &none, "zh");
    assert!(clashes.is_empty(), "{clashes:?}");
    let resolved = resolved(&packaged, &none);
    let origin = Origin::Package("gravity".to_string());
    assert_eq!(
        resolved.get("providers.ag.driver"),
        Some((&Value::Text("gravity/agy".into()), &origin))
    );
    assert_eq!(
        resolved.get("providers.ag.name"),
        Some((&Value::Text("反重力".into()), &origin)),
        "名字照语言挑"
    );
    assert_eq!(
        resolved
            .get("providers.ag.models.g3.window")
            .map(|(value, _)| value),
        Some(&Value::Int(1000))
    );
}

#[test]
fn a_person_overlay_with_the_marker_wins_key_by_key() {
    let overlay = system("[providers.ag]\npackage = \"gravity\"\nname = \"Mine\"\n");
    let (packaged, clashes) = layer(&items(), &[declared("gravity", AG)], &overlay, "en");
    assert!(clashes.is_empty(), "{clashes:?}");
    let resolved = resolved(&packaged, &overlay);
    let (name, origin) = resolved.get("providers.ag.name").expect("有");
    assert_eq!(name, &Value::Text("Mine".into()));
    assert_eq!(origin.layer_name(), "system", "人改的盖住包给的");
    assert_eq!(
        resolved
            .get("providers.ag.driver")
            .map(|(_, origin)| origin.layer_name()),
        Some("package"),
        "没改的照包给的"
    );
}

#[test]
fn a_person_provider_without_the_marker_shadows_the_package_one() {
    // 人原来只写了 key、驱动照目录推的那一家：不能让包接走。
    let own = system("[providers.ag]\nkey = { secret = \"mine\" }\n");
    let (packaged, clashes) = layer(&items(), &[declared("gravity", AG)], &own, "en");
    assert_eq!(
        clashes,
        [Clash::Shadowed {
            provider: "ag".to_string(),
            package: "gravity".to_string()
        }]
    );
    assert!(packaged.parsed.entries.is_empty(), "包带的那一家整家不出现");
    let resolved = resolved(&packaged, &own);
    assert_eq!(resolved.get("providers.ag.driver"), None);
    // 写的是别的包，也算人自己的。
    let other = system("[providers.ag]\npackage = \"other\"\nname = \"X\"\n");
    let (_, clashes) = layer(&items(), &[declared("gravity", AG)], &other, "en");
    assert!(
        matches!(clashes[..], [Clash::Shadowed { .. }]),
        "{clashes:?}"
    );
}

#[test]
fn an_overlay_without_its_package_does_not_count() {
    let overlay = system(
        "[providers.ag]\npackage = \"gravity\"\nname = \"Mine\"\n\n[providers.own]\ndriver = \"openai-chat\"\n",
    );
    let (packaged, clashes) = layer(&items(), &[], &overlay, "en");
    assert!(clashes.is_empty());
    assert!(packaged.masked.contains("providers.ag.name"));
    assert!(packaged.masked.contains("providers.ag.package"));
    assert!(
        !packaged
            .masked
            .iter()
            .any(|key| key.starts_with("providers.own."))
    );
    let resolved = resolved(&packaged, &overlay);
    assert_eq!(
        resolved.get("providers.ag.name"),
        None,
        "包关了、卸了这一家就没有"
    );
    assert!(
        resolved.get("providers.own.driver").is_some(),
        "人自己的照旧"
    );
    assert_eq!(
        miyu_config::key::names(resolved.keys(), "providers.<id>", &[]),
        ["own"]
    );
}

#[test]
fn taken_foreign_and_invalid_are_told() {
    let none = system("");
    let foreign = declared("thief", AG);
    let (packaged, clashes) = layer(
        &items(),
        &[
            foreign,
            declared("gravity", AG),
            declared("later", "[providers.ag]\ndriver = \"openai-chat\"\n"),
        ],
        &none,
        "en",
    );
    assert_eq!(
        clashes[..2],
        [
            Clash::Foreign {
                provider: "ag".to_string(),
                package: "thief".to_string()
            },
            Clash::Taken {
                provider: "ag".to_string(),
                package: "later".to_string(),
                by: "gravity".to_string()
            },
        ]
    );
    assert!(packaged.owners.values().all(|owner| owner == "gravity"));
    let odd = declared(
        "gravity",
        "[providers.ag]\ndriver = \"gravity/agy\"\nprice_multiplier = \"cheap\"\n",
    );
    let (packaged, clashes) = layer(&items(), &[odd], &none, "en");
    assert!(
        matches!(&clashes[..], [Clash::Invalid { keys, .. }] if keys == &["providers.ag.price_multiplier".to_string()]),
        "{clashes:?}"
    );
    assert!(
        packaged.parsed.entries.contains_key("providers.ag.driver"),
        "别的格照收"
    );
}

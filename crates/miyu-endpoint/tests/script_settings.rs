//! 脚本包的配置项（施工 PS-1）：只带脚本的包（bundle）写的 `[settings]` 照 process 包那样登记、挂在「软件包」页这个包那一组；
//! 配置服务交出这个包配置项这一刻的最终值（脚本的 `MIYU_SETTINGS_JSON`）：键去掉包的配置前缀（有组的是 `组.名字`），密钥是解出来
//! 的真值，没设、没默认值的不放；没有配置项的包没有。

use serde_json::json;

use miyu_endpoint::config::{Config, Environment};
use miyu_session::ConfigSource;

use crate::support::*;

/// 一个只带脚本的包：地址有默认值，密钥没有，组里一项有默认值，另一项什么都没有。
const DRAW: &str = "[package]\nname = { en = \"Draw\" }\n\n[settings.endpoint]\ntype = \"url\"\ndefault = \"https://example.com/v1\"\nname = { en = \"Endpoint\" }\n\n[settings.key]\ntype = \"secret\"\nname = { en = \"API key\" }\n\n[settings.style.size]\ntype = \"int\"\ndefault = 512\nname = { en = \"Size\" }\n\n[settings.style.seed]\ntype = \"int\"\nname = { en = \"Seed\" }\n";

/// 照核心起来时那样读包、登记配置项、读配置（只有包的配置项）。
fn config(home: &Home) -> Config {
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root);
    let items = miyu_endpoint::packages::settle(&mut found, &[]);
    Config::load(&home.root, None, items, Environment::of(&[]))
}

#[test]
fn a_bundle_hands_its_scripts_its_own_settings() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    home.write(
        "packages/plain/package.toml",
        "[package]\nname = { en = \"Plain\" }\n",
    );
    home.write("config.toml", "[draw]\nkey = { secret = \"draw\" }\n");
    home.write("secrets.toml", "draw = \"sk-real\"\n");
    let config = config(&home);
    let registered: Vec<(&str, &str, Option<&str>)> = config
        .items()
        .iter()
        .filter(|item| item.key.starts_with("draw."))
        .map(|item| (item.key, item.ui.page, item.ui.package))
        .collect();
    assert_eq!(
        registered,
        [
            ("draw.endpoint", "packages", Some("draw")),
            ("draw.key", "packages", Some("draw")),
            ("draw.style.size", "packages", Some("draw")),
            ("draw.style.seed", "packages", Some("draw")),
        ],
        "登记了，在「软件包」页这个包那一组"
    );
    assert_eq!(
        config
            .package_settings("draw")
            .map(serde_json::Value::Object),
        Some(json!({"endpoint": "https://example.com/v1", "key": "sk-real", "style.size": 512})),
        "键去掉前缀、密钥是真值、没设的不放"
    );
    assert_eq!(config.package_settings("plain"), None, "没有配置项的包没有");
    assert_eq!(config.package_settings("nothing"), None, "没装的没有");
}

/// 写了配置前缀的包（施工 PN-1）：键照前缀起，去掉的也是前缀。
#[test]
fn the_prefix_is_what_is_taken_off() {
    let home = Home::new();
    home.write(
        "packages/miyu-draw/package.toml",
        &DRAW.replace("[package]\n", "[package]\nprefix = \"draw\"\n"),
    );
    home.write(
        "config.toml",
        "[draw]\nendpoint = \"https://other.example.com\"\n",
    );
    let config = config(&home);
    assert_eq!(
        config
            .package_settings("miyu-draw")
            .map(serde_json::Value::Object),
        Some(json!({"endpoint": "https://other.example.com", "style.size": 512}))
    );
}

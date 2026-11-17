//! 设置页上包的「启用」（施工 UX-1，项目主人 2026-11-17）：`config.schema` 里有开关的包一项 `packages.<编号>.enabled`，名字
//! 「启用」、开关、当场生效，排在那个包自己的配置项前面；没有开关的包（界面）没有这一项，关包的 `.off` 一项都没有了。
//! `config.get` 交它这时开没开；`config.set` 改它就是真的起停进程、卸掉装回出厂的内置包，配置文件里一个字都不写。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::extensions::{Program, core, core_with_settings, quick, until_state};
use crate::support::*;

/// 一个自己装的扩展，手动开（同 `package_switch.rs`），带一项配置。
fn relay(program: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Relay\" }}\n\n[command]\nname = \"relay\"\nprogram = \"{program}\"\nabout = {{ en = \"Relay\" }}\n\n[process]\nargs = [\"hello\", \"wait\"]\nstart = \"manual\"\n\n[settings.port]\ntype = \"int\"\nname = {{ en = \"Port\" }}\n"
    )
}

/// 一个程序一定不在的界面：没有开关。
fn screen(home: &Home) {
    home.write(
        "opt/screen/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Screen\" }\n\n[command]\nname = \"screen\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = { en = \"S\" }\n\n[ui]\nopens = [\"config\"]\n",
    );
}

/// `config.set` 改一项。
async fn set(client: &mut Client, n: u8, change: Value) -> Value {
    client
        .call(
            &format!("config-5d1e7a3c9b2f4680-{n}"),
            "config.set",
            json!({"layer": "system", "changes": [change]}),
        )
        .await
}

/// `package.list` 里 `id` 那一项的 `enabled`。
async fn enabled(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["package"] == id)
        .map(|one| one["enabled"].clone())
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

#[tokio::test]
async fn the_settings_switch_is_the_package_switch() {
    let home = Home::new();
    let program = Program::new();
    home.write("opt/relay/package.toml", &relay(&program.name()));
    screen(&home);
    let core = core_with_settings(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let schema = client.call("s", "config.schema", json!({})).await;
    let items = schema["result"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{schema}"));
    let keys: Vec<&str> = items
        .iter()
        .filter_map(|item| item["key"].as_str())
        .collect();
    assert!(
        !keys.iter().any(|key| key.ends_with(".off")),
        "关包的那一项删了：{keys:?}"
    );
    assert!(
        keys.contains(&"packages.screen.enabled"),
        "界面也有开关（施工 UX-1 补：人人有开关）"
    );
    let at = |key: &str| keys.iter().position(|one| *one == key);
    assert!(
        at("packages.relay.enabled") < at("relay.port"),
        "排在包自己的项前面：{keys:?}"
    );
    let item = &items[at("packages.relay.enabled").expect("有它")];
    assert_eq!(
        (
            item["name"].clone(),
            item["type"].clone(),
            item["applies"].clone(),
            item["group"].clone(),
            item["package"].clone(),
            item["page"].clone()
        ),
        (
            json!("启用"),
            json!("bool"),
            json!("now"),
            json!("relay"),
            json!("relay"),
            json!("packages")
        ),
        "{item}"
    );
    let asked = client
        .call(
            "s",
            "config.schema",
            json!({"keys": ["packages.relay.enabled"]}),
        )
        .await;
    assert_eq!(
        asked["result"]["items"][0]["key"], "packages.relay.enabled",
        "{asked}"
    );

    let got = client
        .call(
            "g",
            "config.get",
            json!({"keys": ["packages.relay.enabled"]}),
        )
        .await;
    assert_eq!(
        got["result"]["items"]["packages.relay.enabled"],
        json!({"origin": {"layer": "package"}, "value": false}),
        "{got}"
    );

    let on = set(
        &mut client,
        1,
        json!({"key": "packages.relay.enabled", "value": true, "expect": {}}),
    )
    .await;
    assert_eq!(
        on["result"]["keys"]["packages.relay.enabled"],
        json!({"applies": "now", "effective": true, "origin": {"layer": "package"}, "value": true}),
        "{on}"
    );
    until_state(&mut client, "relay", |one| one["state"] == "running").await;
    assert_eq!(enabled(&mut client, "relay").await, true);

    let off = set(
        &mut client,
        2,
        json!({"key": "packages.relay.enabled", "input": "false"}),
    )
    .await;
    assert_eq!(
        off["result"]["keys"]["packages.relay.enabled"]["effective"], false,
        "{off}"
    );
    until_state(&mut client, "relay", |one| one["state"] == "off").await;
    assert_eq!(enabled(&mut client, "relay").await, false);

    let written = std::fs::read_to_string(home.root.config_file()).unwrap_or_default();
    assert!(!written.contains("enabled"), "配置文件里不写：{written}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn switches_mix_with_other_changes_and_odd_writes_are_refused() {
    let home = Home::new();
    screen(&home);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    // 出厂的内置包关是卸掉；同一条里别的项照常写进配置。
    let both = client
        .call(
            "m",
            "config.set",
            json!({"layer": "system", "changes": [
                {"key": "packages.base-memory.enabled", "value": false},
                {"key": "ui.startup", "value": "recent"}
            ]}),
        )
        .await;
    assert_eq!(
        both["result"]["keys"]["packages.base-memory.enabled"]["effective"], false,
        "{both}"
    );
    assert_eq!(
        both["result"]["keys"]["ui.startup"]["effective"], "recent",
        "{both}"
    );
    assert_eq!(enabled(&mut client, "base-memory").await, false);
    let on = set(
        &mut client,
        1,
        json!({"key": "packages.base-memory.enabled", "value": true}),
    )
    .await;
    assert_eq!(
        on["result"]["keys"]["packages.base-memory.enabled"]["effective"], true,
        "{on}"
    );
    assert_eq!(enabled(&mut client, "base-memory").await, true);

    for (n, change, reason) in [
        (
            2,
            json!({"key": "packages.base-memory.enabled", "value": "yes"}),
            "bad_params",
        ),
        (
            3,
            json!({"key": "packages.base-memory.enabled", "unset": true}),
            "bad_params",
        ),
        (
            4,
            json!({"key": "packages.nothing.enabled", "value": true}),
            "unknown_config_key",
        ),
        (
            5,
            json!({"key": "packages.base-memory.off", "value": false}),
            "unknown_config_key",
        ),
    ] {
        let reply = set(&mut client, n, change.clone()).await;
        assert_eq!(
            reply["error"]["data"]["reason"], reason,
            "{change}：{reply}"
        );
    }
}

//! 扩展提供的模型驱动（施工 MD-1，设计 66 第四、九、十节，core1 11-07 定 A）：真核心走一遍。
//!
//! - 装着、声明了 `[[driver]]` 的包，「一家供应商的驱动」的下拉里多一项 `<包>/<驱动名>`，名字是「包的名字：驱动名」；
//! - 写了没装的：值照留，`miyu check`、读回来都报警告 `unknown_driver`；`config.set` 照收；改回内置的、删掉这一格照常；
//! - `miyu check` 另报：包没批 `model.driver`（错误，批过的不报）、驱动要 key 这一家没配（错误）、驱动不要 key 这一家却配了（警告）；
//! - 装之前那一屏多一格 `drivers`，能力里有 `model.driver`。

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::*;

/// 包 `id` 的清单：要 `model.driver`，声明驱动 `ag`，要不要 key 照 `key`。
fn manifest(name: &str, key: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"{name}\" }}\n\n[command]\nname = \"{}\"\nprogram = \"nowhere\"\nabout = {{ en = \"D\" }}\n\n[process]\nstart = \"manual\"\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"ag\"\nkey = \"{key}\"\n",
        name.to_lowercase()
    )
}

/// 两个包：`gravity` 的驱动要 key，`plainer` 的不要。
fn packages(home: &Home) {
    home.write("opt/gravity/package.toml", &manifest("Gravity", "required"));
    home.write("opt/plainer/package.toml", &manifest("Plainer", "none"));
}

/// 一份核心：模型那几项照核心起来时的做法（`miyu-core` 的 `Packaged::all`）接上装着的两个驱动。
async fn connected(home: &Home) -> Client {
    let installed = ["gravity/ag".to_string(), "plainer/ag".to_string()];
    let items: Vec<miyu_config::Item> = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_models::settings::ProviderSettings::ITEMS,
        miyu_models::settings::ModelSettings::ITEMS,
    ]
    .concat()
    .iter()
    .map(|item| miyu_config::driver_ref::with_installed(item, &installed))
    .collect();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = home
        .core_full(&Script::new([]), Catalog::default(), None, TOKEN)
        .with_config(config);
    let mut client = Client::connect(std::sync::Arc::new(core));
    client.hello().await;
    client
}

/// 检查报的每一处：代码、级别、种类。
async fn checked(client: &mut Client) -> Vec<(String, String, String)> {
    let reply = client.call("k", "check", json!({})).await;
    reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|problem| {
            let text = |key: &str| problem[key].as_str().unwrap_or_default().to_string();
            (text("code"), text("level"), text("kind"))
        })
        .collect()
}

#[tokio::test]
async fn check_reports_unknown_unapproved_and_key_mismatches() {
    let home = Home::new();
    packages(&home);
    home.write(
        "etc/config.toml",
        "[providers.g]\ndriver = \"gravity/ag\"\nbase_url = \"https://g.invalid\"\n\n\
         [providers.p]\ndriver = \"plainer/ag\"\nbase_url = \"https://p.invalid\"\nkey = { env = \"P_KEY\" }\n\n\
         [providers.x]\ndriver = \"gone/drv\"\nbase_url = \"https://x.invalid\"\n",
    );
    // `plainer` 在开关文件里批过 `model.driver`，`gravity` 没批。
    home.write(
        "etc/extensions.json",
        "{\"version\":1,\"on\":{},\"approved\":{\"plainer\":[\"model.driver\"]}}\n",
    );
    let mut client = connected(&home).await;
    let mut problems = checked(&mut client).await;
    problems.retain(|(code, _, _)| code.starts_with("driver_") || code == "unknown_driver");
    problems.sort();
    let owned =
        |code: &str, level: &str| (code.to_string(), level.to_string(), "config".to_string());
    assert_eq!(
        problems,
        [
            owned("driver_key_missing", "error"),
            owned("driver_key_unused", "warning"),
            owned("driver_unapproved", "error"),
            owned("unknown_driver", "warning"),
        ],
        "g 要 key 没配、p 不要 key 却配了、gravity 没批（plainer 批过）、gone 没装"
    );
}

#[tokio::test]
async fn the_dropdown_lists_installed_drivers_and_set_keeps_unknown_ones() {
    let home = Home::new();
    packages(&home);
    let mut client = connected(&home).await;
    let schema = client
        .call(
            "s",
            "config.schema",
            json!({"keys": ["providers.<id>.driver"]}),
        )
        .await;
    let options: Vec<(String, String)> = schema["result"]["items"][0]["options"]
        .as_array()
        .unwrap_or_else(|| panic!("{schema}"))
        .iter()
        .map(|option| {
            let text = |key: &str| option[key].as_str().unwrap_or_default().to_string();
            (text("value"), text("name"))
        })
        .collect();
    assert!(
        options.contains(&("gravity/ag".to_string(), "Gravity: ag".to_string())),
        "{options:?}"
    );
    assert!(options.iter().any(|(value, _)| value == "openai-chat"));
    let set = |value: Value| json!({"layer": "system", "changes": [{"key": "providers.x.driver", "input": value}]});
    for (n, value) in [json!("gone/drv"), json!("gravity/ag"), json!("openai-chat")]
        .into_iter()
        .enumerate()
    {
        let reply = client
            .call(&format!("set{n}"), "config.set", set(value.clone()))
            .await;
        assert!(reply.get("error").is_none(), "{value}：{reply}");
    }
    let reply = client
        .call("bad", "config.set", set(json!("Gone/Drv")))
        .await;
    assert!(reply.get("error").is_some(), "别的写法照旧不收：{reply}");
    let reply = client
        .call("again", "config.set", set(json!("gone/drv")))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let problems = checked(&mut client).await;
    assert!(
        problems
            .iter()
            .any(|(code, level, _)| code == "unknown_driver" && level == "warning"),
        "读回来照记警告：{problems:?}"
    );
}

#[tokio::test]
async fn the_install_screen_names_the_drivers_and_the_capability() {
    let home = Home::new();
    packages(&home);
    let mut client = connected(&home).await;
    let reply = client
        .call("i", "package.info", json!({"package": "gravity"}))
        .await;
    let info = &reply["result"];
    assert_eq!(info["provides"]["drivers"], json!(["ag"]), "{reply}");
    let capabilities: Vec<&str> = info["capabilities"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|capability| capability["id"].as_str())
        .collect();
    assert_eq!(capabilities, ["model.driver"]);
}

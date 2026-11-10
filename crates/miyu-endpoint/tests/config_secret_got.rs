//! 密钥项写错时不回显原值（施工 SEC-1，`docs/blueprint/config.md`「报错」`got` 那一行）：`config.set` 照字（`value`）、照人敲的
//! （`input`）写明文，配置文件里手写明文，`config.get`、`config.check`、`check` 报的问题里都没有那一串字，只说写的是几个字符的字；
//! 不是密钥的项照旧照抄。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 假的密钥：24 个字符。
const FAKE: &str = "sk-test-0123456789abcdef";

/// 出厂的类型是密钥的一项：供应商的 key（`miyu_models::settings::ProviderSettings`）。
const KEY: &str = "providers.dev.key";

/// 连上一个清单带着供应商那几项的核心，握过手。
async fn client(home: &Home) -> Client {
    let items = miyu_models::settings::ProviderSettings::ITEMS;
    let mut client = Client::connect(home.core_with_items(&Script::new([]), None, &[], items));
    client.hello().await;
    client
}

/// 拒绝里的问题。
fn problems(reply: &Value) -> &Vec<Value> {
    assert_eq!(reason(reply), Some("config_invalid"), "{reply}");
    reply["error"]["data"]["problems"]
        .as_array()
        .expect("有问题")
}

/// 整条回应里没有 `secret` 这串字，也没有它的前一半。
fn never_echoes(reply: &Value, secret: &str) {
    let whole = reply.to_string();
    let half = &secret[..secret.len() / 2];
    assert!(!whole.contains(half), "回显了密钥：{whole}");
}

#[tokio::test]
async fn a_plain_key_set_on_a_secret_item_is_refused_without_echoing_it() {
    let home = Home::new();
    let mut client = client(&home).await;
    let by_value = client
        .call(
            "c1",
            "config.set",
            json!({"layer": "system", "changes": [{"key": KEY, "value": FAKE}]}),
        )
        .await;
    let problem = &problems(&by_value)[0];
    assert_eq!(problem["code"], "wrong_type");
    assert_eq!(problem["key"], KEY);
    assert_eq!(problem["got"], "24 个字符的字", "{by_value}");
    assert_eq!(
        problem["message"],
        format!("{KEY} 要写 {{ secret = \"…\" }} 或 {{ env = \"…\" }}，写的是 24 个字符的字。")
    );
    never_echoes(&by_value, FAKE);
    // 人敲的（`miyu config set`）同样不回显；超过 80 个字的也不留下截出来的那一段。
    let long = format!("sk-test-{}", "0123456789abcdef".repeat(7));
    let by_input = client
        .call(
            "c2",
            "config.set",
            json!({"layer": "system", "changes": [{"key": KEY, "input": long}]}),
        )
        .await;
    assert_eq!(
        problems(&by_input)[0]["got"],
        "120 个字符的字",
        "{by_input}"
    );
    never_echoes(&by_input, &long);
    // 写成数组、对象的说是几项的列表、一张表。
    let listed = client
        .call(
            "c3",
            "config.set",
            json!({"layer": "system", "changes": [{"key": KEY, "value": [FAKE, FAKE]}]}),
        )
        .await;
    assert_eq!(problems(&listed)[0]["got"], "2 项的列表", "{listed}");
    never_echoes(&listed, FAKE);
    let table = client
        .call(
            "c4",
            "config.set",
            json!({"layer": "system", "changes": [{"key": KEY, "value": {"key": FAKE}}]}),
        )
        .await;
    assert_eq!(problems(&table)[0]["got"], "一张表", "{table}");
    never_echoes(&table, FAKE);
}

#[tokio::test]
async fn a_plain_key_in_the_file_is_not_echoed_by_get_check_or_config_check() {
    let home = Home::new();
    let text = format!("[providers.dev]\nkey = \"{FAKE}\"\n");
    home.write("etc/config.toml", &text);
    let mut client = client(&home).await;
    let got = client.call("g1", "config.get", json!({})).await;
    let found = got["result"]["problems"].as_array().expect("有问题");
    let problem = found.iter().find(|problem| problem["key"] == KEY);
    let problem = problem.unwrap_or_else(|| panic!("报了这一项：{got}"));
    assert_eq!(problem["code"], "wrong_type");
    assert_eq!(problem["got"], "24 个字符的字", "{got}");
    never_echoes(&got, FAKE);
    let checked = client.call("k1", "check", json!({})).await;
    assert!(checked["result"]["problems"].is_array(), "{checked}");
    never_echoes(&checked, FAKE);
    let config_check = client
        .call(
            "k2",
            "config.check",
            json!({"layer": "system", "text": text}),
        )
        .await;
    assert_eq!(
        config_check["result"]["problems"][0]["got"], "24 个字符的字",
        "{config_check}"
    );
    never_echoes(&config_check, FAKE);
}

#[tokio::test]
async fn a_misspelled_secret_key_does_not_echo_its_value() {
    let home = Home::new();
    let text = format!("[providers.dev]\nkye = \"{FAKE}\"\n");
    home.write("etc/config.toml", &text);
    let mut client = client(&home).await;
    let got = client.call("g1", "config.get", json!({})).await;
    let found = got["result"]["problems"].as_array().expect("有问题");
    let problem = found
        .iter()
        .find(|problem| problem["key"] == "providers.dev.kye");
    let problem = problem.unwrap_or_else(|| panic!("报了这一项：{got}"));
    assert_eq!(problem["code"], "unknown_key");
    assert_eq!(problem["got"], "24 个字符的字", "{got}");
    never_echoes(&got, FAKE);
    let checked = client
        .call(
            "k1",
            "config.check",
            json!({"layer": "system", "text": text}),
        )
        .await;
    assert_eq!(
        checked["result"]["problems"][0]["code"], "unknown_key",
        "{checked}"
    );
    never_echoes(&checked, FAKE);
}

#[tokio::test]
async fn a_key_written_where_a_group_belongs_is_not_echoed() {
    let home = Home::new();
    let text = format!("[providers]\ndev = \"{FAKE}\"\n");
    home.write("etc/config.toml", &text);
    let mut client = client(&home).await;
    let got = client.call("g1", "config.get", json!({})).await;
    let found = got["result"]["problems"].as_array().expect("有问题");
    let problem = found
        .iter()
        .find(|problem| problem["key"] == "providers.dev");
    let problem = problem.unwrap_or_else(|| panic!("报了这一组：{got}"));
    assert_eq!(problem["code"], "wrong_type");
    assert_eq!(problem["got"], "24 个字符的字", "{got}");
    never_echoes(&got, FAKE);
    let checked = client
        .call(
            "k1",
            "config.check",
            json!({"layer": "system", "text": text}),
        )
        .await;
    never_echoes(&checked, FAKE);
}

#[tokio::test]
async fn a_wrong_value_on_an_item_that_is_not_a_secret_is_still_echoed() {
    let home = Home::new();
    let mut client = client(&home).await;
    let reply = client
        .call(
            "c1",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "permission.start_read_only", "value": "yes"}]}),
        )
        .await;
    let problem = &problems(&reply)[0];
    assert_eq!(problem["code"], "wrong_type");
    assert_eq!(problem["got"], "\"yes\"");
    assert_eq!(
        problem["message"],
        "permission.start_read_only 要写 true 或 false，写的是 \"yes\"。改成 permission.start_read_only = true。"
    );
}

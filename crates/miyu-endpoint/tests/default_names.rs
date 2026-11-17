//! 默认人格、默认预设指着没有的（施工 CK-1）：`persona.default`、`preset.default` 在 `config.schema` 里是 `persona`、`preset`，
//! 同终端会话那两项；写了现在没有的，`check` 报警告 `unknown_persona`、`unknown_preset`，`config.set` 照收、回应里带上同样的
//! 警告（`problems`），照连接的语言说。出厂的预设 `full` 不报。

use serde_json::{Value, json};

use crate::support::*;

/// 握手：英文。
async fn hello_in_english(client: &mut Client) {
    let reply = client
        .call(
            "hello-1",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "locale": "en",
                "caps": {"input": true},
                "token": TOKEN,
            }),
        )
        .await;
    assert!(reply.get("result").is_some(), "{reply}");
}

/// 一份问题里配置的那几条：键、代码、那一句。
fn warned(problems: &Value) -> Vec<(String, String, String)> {
    problems
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|problem| problem["level"] == "warning")
        .map(|problem| {
            let text = |field: &str| problem[field].as_str().unwrap_or_default().to_string();
            (text("key"), text("code"), text("message"))
        })
        .collect()
}

#[tokio::test]
async fn check_warns_when_the_defaults_name_what_is_not_there() {
    let home = Home::new();
    home.write(
        "etc/config.toml",
        "[persona]\ndefault = \"ghost\"\n\n[preset]\ndefault = \"gone\"\n",
    );
    let mut client = Client::connect(home.core(&miyu_session::testkit::Script::new([])));
    hello_in_english(&mut client).await;
    let reply = client
        .call(
            "t1",
            "config.schema",
            json!({"keys": ["persona.default", "preset.default"]}),
        )
        .await;
    let types: Vec<&Value> = reply["result"]["items"]
        .as_array()
        .expect("有项")
        .iter()
        .map(|item| &item["type"])
        .collect();
    assert_eq!(types, [&json!("persona"), &json!("preset")], "{reply}");
    let checked = client.call("t2", "check", json!({})).await;
    let found: Vec<(String, String)> = warned(&checked["result"]["problems"])
        .into_iter()
        .map(|(key, code, _)| (key, code))
        .collect();
    assert_eq!(
        found,
        [
            ("persona.default".to_string(), "unknown_persona".to_string()),
            ("preset.default".to_string(), "unknown_preset".to_string()),
        ],
        "{checked}"
    );
}

#[tokio::test]
async fn setting_a_default_that_is_not_there_warns_in_the_reply() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&miyu_session::testkit::Script::new([])));
    hello_in_english(&mut client).await;
    let reply = client
        .call(
            "t1",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "preset.default", "input": "gone"}]}),
        )
        .await;
    assert_eq!(
        warned(&reply["result"]["problems"]),
        [(
            "preset.default".to_string(),
            "unknown_preset".to_string(),
            "preset.default names preset gone, which doesn't exist yet; it works once the preset is created.".to_string(),
        )],
        "照收，回应里带警告：{reply}"
    );
    assert_eq!(reply["result"]["keys"]["preset.default"]["value"], "gone");
    // 出厂的预设、有的人格不报。
    home.write("root/personas/cat/persona.toml", "[voice]\n");
    let reply = client
        .call(
            "t2",
            "config.set",
            json!({"layer": "system", "changes": [
                {"key": "preset.default", "input": "full"},
                {"key": "persona.default", "input": "cat"},
            ]}),
        )
        .await;
    assert!(reply.get("result").is_some(), "{reply}");
    assert_eq!(reply["result"].get("problems"), None, "{reply}");
}

//! 每人一份密钥（施工 Y-2 下下中中，2026-10-24 项目主人定「成员只继承系统层」）：成员存的进他自己那一份，写不了系统那一份；
//! 没写 `layer` 的，管理员存系统那一份、成员存自己的；列出来的是他用得上的两份；试供应商时成员用得上系统那一份和自己的，
//! 用不上机主自己的；写配置成员只写得了个人设置。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::login::*;
use crate::support::*;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

async fn store(client: &mut Client, params: Value) -> Value {
    client.call("s", "secret.set", params).await
}

/// 照 key 是 `{secret = name}` 的候选试一回，交回密钥取没取到：取不到的报 `no_model`（引用的密钥还没设）；取到的往下走
/// （测试核心没有发请求的客户端，停在 `other`）。
async fn found(client: &mut Client, name: &str) -> bool {
    let reply = client
        .call(
            "t",
            "provider.test",
            json!({"candidate": {"driver": "openai-chat", "base_url": "http://127.0.0.1:9/v1",
                "key": {"secret": name}}, "model": "m"}),
        )
        .await;
    reply["result"]["error"]["class"] != "no_model"
}

fn names(listed: &Value) -> Vec<(String, Value)> {
    listed["result"]["secrets"]
        .as_array()
        .expect("列得出")
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap_or("").to_string(),
                row["layer"].clone(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_member_keeps_his_own_keys_and_borrows_only_the_system_ones() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let mut bob = member(&core, "bob").await;
    let read = |relative: &str| std::fs::read_to_string(home.root.path().join(relative)).ok();

    // 没写 layer：机主存系统那一份，成员存他自己的。
    let shared = store(&mut owner, json!({"name": "shared", "value": FAKE})).await;
    assert!(shared.get("result").is_some(), "{shared}");
    assert!(read("system/secrets.toml").is_some_and(|text| text.contains("shared")));
    let private = store(
        &mut owner,
        json!({"name": "private", "value": FAKE, "layer": "personal"}),
    )
    .await;
    assert!(private.get("result").is_some(), "{private}");
    assert!(read("home/alice/secrets.toml").is_some_and(|text| text.contains("private")));
    let his = store(&mut bob, json!({"name": "his", "value": FAKE})).await;
    assert!(his.get("result").is_some(), "{his}");
    let file = read("home/bob/secrets.toml").expect("写进了他自己那一份");
    assert!(file.contains("his"), "{file}");
    assert!(
        !read("system/secrets.toml")
            .unwrap_or_default()
            .contains("his")
    );

    // 系统那一份成员写不了、删不了。
    for (method, params) in [
        (
            "secret.set",
            json!({"name": "x", "value": FAKE, "layer": "system"}),
        ),
        (
            "secret.delete",
            json!({"name": "shared", "layer": "system"}),
        ),
    ] {
        let refused = bob.call("r", method, params).await;
        assert_eq!(
            refused["error"]["data"]["reason"], "not_permitted",
            "{method}: {refused}"
        );
    }
    assert!(read("system/secrets.toml").is_some_and(|text| text.contains("shared")));

    // 列的是他用得上的两份，带是哪一份。
    let listed = bob.call("l", "secret.list", json!({})).await;
    assert_eq!(
        names(&listed),
        [
            ("his".to_string(), json!("personal")),
            ("shared".to_string(), json!("system")),
        ],
        "{listed}"
    );
    let listed = owner.call("l", "secret.list", json!({})).await;
    assert_eq!(
        names(&listed),
        [
            ("private".to_string(), json!("personal")),
            ("shared".to_string(), json!("system")),
        ],
        "{listed}"
    );

    // 试供应商：机主自己的成员取不到；系统那一份和他自己的取得到。
    assert!(!found(&mut bob, "private").await);
    assert!(found(&mut owner, "private").await);
    assert!(found(&mut bob, "shared").await);
    assert!(found(&mut bob, "his").await);

    // 删他自己的不碰别人的。
    let deleted = bob.call("d", "secret.delete", json!({"name": "his"})).await;
    assert_eq!(deleted["result"], json!({}), "{deleted}");
    assert!(
        !read("home/bob/secrets.toml")
            .unwrap_or_default()
            .contains("his")
    );
    assert!(read("system/secrets.toml").is_some_and(|text| text.contains("shared")));
}

/// 模型列表照这个人那一份配置列：机主个人设置里配的供应商成员看不到，系统配置里的大家都看得到。
#[tokio::test]
async fn the_model_list_follows_whose_settings() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.shared]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\n",
    );
    home.write(
        "home/alice/settings.toml",
        "[providers.private]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\n",
    );
    let core = home.core_with_items(
        &Script::new([]),
        None,
        &[],
        miyu_models::settings::ProviderSettings::ITEMS,
    );
    let mut owner = local(&core).await;
    let mut bob = member(&core, "bob").await;
    let ids = |listed: &Value| -> Vec<String> {
        listed["result"]["providers"]
            .as_array()
            .expect("列得出")
            .iter()
            .filter_map(|row| row["id"].as_str().map(str::to_string))
            .filter(|id| id == "shared" || id == "private")
            .collect()
    };
    let mine = owner.call("m", "model.list", json!({})).await;
    assert_eq!(ids(&mine), ["private", "shared"], "{mine}");
    let his = bob.call("m", "model.list", json!({})).await;
    assert_eq!(ids(&his), ["shared"], "{his}");
}

/// 核心起来以前就在的个人密钥文件照读；他的个人设置引用的密钥照他用得上的两份查：自己的、系统的算设了，机主自己的算没设。
#[tokio::test]
async fn his_settings_check_against_his_two_shelves() {
    let home = Home::new();
    let items = miyu_models::settings::ProviderSettings::ITEMS;
    // 先注册 bob，再换一份核心起来：他那一份是新核心头一次开的时候读的。
    drop(
        member(
            &home.core_with_items(&Script::new([]), None, &[], items),
            "bob",
        )
        .await,
    );
    home.write("system/secrets.toml", &format!("shared = \"{FAKE}\"\n"));
    home.write(
        "home/alice/secrets.toml",
        &format!("private = \"{FAKE}\"\n"),
    );
    home.write("home/bob/secrets.toml", &format!("pre = \"{FAKE}\"\n"));
    home.write(
        "home/bob/settings.toml",
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\nkey = { secret = \"pre\" }\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\nkey = { secret = \"shared\" }\n\n\
         [providers.c]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\nkey = { secret = \"private\" }\n",
    );
    let core = home.core_with_items(&Script::new([]), None, &[], items);
    let mut bob = Client::connect(core);
    let hello = hello_with(
        &mut bob,
        json!({"user": "bob", "password": "correct horse"}),
    )
    .await;
    assert_eq!(hello["result"]["account"], "bob", "{hello}");
    let listed = bob.call("l", "secret.list", json!({})).await;
    assert_eq!(
        names(&listed),
        [
            ("pre".to_string(), json!("personal")),
            ("private".to_string(), Value::Null),
            ("shared".to_string(), json!("system")),
        ],
        "{listed}"
    );
    let got = bob.call("g", "config.get", json!({})).await;
    let unknown: Vec<&str> = got["result"]["problems"]
        .as_array()
        .expect("{got}")
        .iter()
        .filter(|problem| problem["code"] == "unknown_secret")
        .filter_map(|problem| problem["key"].as_str())
        .collect();
    assert_eq!(unknown, ["providers.c.key"], "{got}");
    assert!(found(&mut bob, "pre").await);
}

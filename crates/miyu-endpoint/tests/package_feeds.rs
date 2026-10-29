//! 别的软件源（施工 U23 下下，设计 `37-软件源.md` 第五节）：配置里 `packages.sources.<名字>` 写地址和公钥就是信任它。
//! 搜得到、照它的公钥验、装进装上的那一层、本地库记着源名。源从配置里删了，照它装的包升级时进 `gone`，不换到别的源上。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_endpoint::settings::{SOURCE_FEEDS, SourceSettings};
use miyu_http::testkit::Server;
use miyu_session::testkit::Script;
use miyu_source::sign::{self, PublicKey};
use miyu_tool::Catalog;

use crate::support::source::{bytes, packed, serving, sha256, signed_entries};
use crate::support::*;

/// 一对钥匙：私钥、公钥的 base64。
fn pair() -> (String, String) {
    let made = sign::generate().expect("生成得了");
    (made.secret, made.public)
}

/// 系统配置里官方的软件源是 `official`、另写 `extra` 的核心；官方的照 `keys` 验。
fn core_with(home: &Home, (official, extra): (&str, &str), keys: Vec<PublicKey>) -> Arc<Core> {
    home.write(
        "config.toml",
        &format!("[packages]\nsource = \"{official}\"\n{extra}"),
    );
    let items = [SourceSettings::ITEMS, SOURCE_FEEDS].concat();
    let config = Config::load(&home.root, None, items, Environment::of(&[]));
    Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_source_keys(keys),
    )
}

/// 索引里一条照真的包文件写的：原址在 `base` 的 `files/` 下。
fn hosted(id: &str, version: &str, file: &(String, Vec<u8>), base: &str) -> Value {
    json!({"id": id, "version": version, "name": {"en": id}, "origin": format!("github.com/friend/{id}"),
           "official": true,
           "files": [{"url": format!("{base}/files/{}", file.0), "size": file.1.len(), "sha256": sha256(&file.1)}]})
}

/// 只有一个包 `id` 的官方索引，签好、照 `serving` 回。
async fn official_with(id: &str, secret: &str) -> Server {
    let entry = json!({"id": id, "version": "1.0", "name": {"en": id}, "origin": format!("github.com/miyu-agent/{id}"),
                       "official": true, "files": [{"url": "https://example.invalid/x.miyu", "sha256": "ab", "size": 1}]});
    Server::start(serving(&signed_entries(
        "2026-10-28T03:00:00Z",
        &[entry],
        secret,
    )))
    .await
}

/// `client` 在设置里加源 `name`：地址 `url`，公钥 `key`。
async fn add_source(client: &mut Client, name: &str, url: &str, key: &str) {
    let reply = client
        .call(
            "s",
            "config.set",
            json!({"layer": "system", "changes": [
                {"key": format!("packages.sources.{name}.url"), "value": url},
                {"key": format!("packages.sources.{name}.key"), "value": key},
            ]}),
        )
        .await;
    assert!(reply.get("result").is_some(), "{reply}");
}

fn reason(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

/// 搜 `query` 的每一行：编号、源、是不是官方的。
async fn found(client: &mut Client, query: &str) -> Vec<(String, String, bool)> {
    let reply = client
        .call("q", "package.search", json!({"query": query}))
        .await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|row| {
            (
                row["package"].as_str().unwrap_or_default().to_string(),
                row["source"].as_str().unwrap_or_default().to_string(),
                row["official"].as_bool().unwrap_or_default(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_source_added_in_settings_is_searched_installed_and_remembered() {
    let home = Home::new();
    let (official_secret, official_public) = pair();
    let official = official_with("base", &official_secret).await;
    let (friend_secret, friend_public) = pair();
    let file = packed(&home, "kit", "1.0", &[]);
    // 包文件放在另一台假服务器上：索引里写的原址指到它。
    let files = Server::start(vec![bytes(&file.1)]).await;
    let friends = Server::start(serving(&signed_entries(
        "2026-10-28T03:00:00Z",
        &[hosted("kit", "1.0", &file, &files.base_url)],
        &friend_secret,
    )))
    .await;
    let keys = vec![PublicKey::from_base64(&official_public).expect("读得成")];
    let core = core_with(&home, (&format!("{}/", official.base_url), ""), keys);
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    add_source(
        &mut owner,
        "friends",
        &format!("{}/", friends.base_url),
        &friend_public,
    )
    .await;

    assert_eq!(
        found(&mut owner, "kit").await,
        [("kit".to_string(), "friends".to_string(), false)],
        "别的源里的不算官方"
    );
    let synced = owner.call("s", "package.sync", json!({})).await;
    assert_eq!(
        synced["result"]["sources"][0]["source"], "friends",
        "{synced}"
    );
    assert_eq!(synced["result"]["sources"][0]["packages"], 1, "{synced}");

    let installed = owner
        .call(
            "i",
            "package.install",
            json!({"source": "kit", "from": "friends"}),
        )
        .await;
    assert_eq!(installed["result"]["layer"], "home", "{installed}");
    assert!(home.root.path().join("packages/kit/package.toml").is_file());
    let desc = std::fs::read_to_string(home.root.path().join("state/packages/.local/kit/desc"))
        .expect("记进本地库");
    assert!(desc.contains("\"feed\":\"friends\""), "{desc}");

    // 照装的时候那个源比：版本一样，不升、也不算没有了。
    let upgrades = owner.call("u", "package.upgrades", json!({})).await;
    assert_eq!(upgrades["result"]["gone"], json!([]), "{upgrades}");
    assert_eq!(upgrades["result"]["upgrades"], json!([]), "{upgrades}");
    // 源删了：照它装的包升级时进 gone，不换到别的源上。
    let removed = owner
        .call(
            "r",
            "config.set",
            json!({"layer": "system", "changes": [
                {"key": "packages.sources.friends.url", "unset": true},
                {"key": "packages.sources.friends.key", "unset": true},
            ]}),
        )
        .await;
    assert!(removed.get("result").is_some(), "{removed}");
    let upgrades = owner.call("u", "package.upgrades", json!({})).await;
    assert_eq!(upgrades["result"]["gone"], json!(["kit"]), "{upgrades}");
    let again = owner
        .call(
            "o",
            "package.install",
            json!({"source": "kit", "from": "friends"}),
        )
        .await;
    assert_eq!(reason(&again), Some("unknown_source"), "{again}");
}

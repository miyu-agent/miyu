//! 头自己的设置（施工 HP-1，`docs/blueprint/protocol.md`「头自己的设置」）：`prefs.get` 交整份，没有的是 `{}`；`prefs.set` 照
//! 顶层的键并进去、`null` 删掉，交回并好的整份，存在那个包的状态目录里的 `prefs.json`；写成 JSON 超过 64 KiB 的
//! `prefs_too_big`，什么都不改；订了这个包的别的连接收到 `prefs.changed`，改的那一个不收。只有装着的界面包有。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 一个界面包。
const PANE: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Pane\" }\n\n[ui]\n";

/// 上限：整份写成 JSON 最多这么多字节。
const LIMIT: usize = 64 * 1024;

/// 起核心、连上一个头，装上界面包 `pane`。
async fn with_pane(home: &Home) -> (Arc<miyu_endpoint::Core>, Client) {
    let core = home.core(&Script::new([]));
    let mut head = Client::connect(Arc::clone(&core));
    head.hello().await;
    let folder = home.work.join("pane");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), PANE).expect("写得进");
    let installed = head
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "pane", "{installed}");
    (core, head)
}

/// 拒的原因码。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

/// `client` 这时手里、接下来一小会儿到的推送里的 `prefs.changed`。
async fn heard(client: &mut Client) -> Vec<Value> {
    let mut all = Vec::new();
    while let Some(message) = client.next_within(Duration::from_millis(300)).await {
        if message["method"] == "prefs.changed" {
            all.push(message["params"].clone());
        }
    }
    all
}

/// 等到 `client` 收到一条 `prefs.changed`：别的跳过，最多 60 秒。
async fn changed(client: &mut Client) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到 prefs.changed"));
        if message["method"] == "prefs.changed" {
            return message["params"].clone();
        }
    }
}

#[tokio::test]
async fn set_merges_by_top_level_key_and_null_deletes() {
    let home = Home::new();
    let (_core, mut head) = with_pane(&home).await;
    let empty = head
        .call("g", "prefs.get", json!({"package": "pane"}))
        .await;
    assert_eq!(
        empty["result"],
        json!({"values": {}}),
        "没有的是空的：{empty}"
    );

    let first = head
        .call(
            "s1",
            "prefs.set",
            json!({"package": "pane", "values": {"skin": "paper", "mascot": {"on": true, "size": 2}}}),
        )
        .await;
    assert_eq!(
        first["result"],
        json!({"values": {"skin": "paper", "mascot": {"on": true, "size": 2}}}),
        "{first}"
    );
    let second = head
        .call(
            "s2",
            "prefs.set",
            json!({"package": "pane", "values": {"skin": null, "mascot": {"on": false}, "scheme": "dark"}}),
        )
        .await;
    let now = json!({"values": {"mascot": {"on": false}, "scheme": "dark"}});
    assert_eq!(second["result"], now, "顶层的键整个换，null 删掉：{second}");
    let nothing = head
        .call("s3", "prefs.set", json!({"package": "pane", "values": {}}))
        .await;
    assert_eq!(nothing["result"], now, "什么都不改的照样交回整份");
    let read = head
        .call("g2", "prefs.get", json!({"package": "pane"}))
        .await;
    assert_eq!(read["result"], now, "{read}");

    let dir = home.root.package_state("pane");
    let file = std::fs::read_to_string(dir.join("prefs.json")).expect("存在包的状态目录里");
    let stored: Value = serde_json::from_str(&file).expect("是 JSON");
    assert_eq!(stored, now["values"], "文件里就是那一份");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("读得了")
        .map(|entry| {
            entry
                .expect("读得了")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["prefs.json"], "不留临时文件");

    let again = home.core(&Script::new([]));
    let mut later = Client::connect(again);
    later.hello().await;
    let reread = later
        .call("g3", "prefs.get", json!({"package": "pane"}))
        .await;
    assert_eq!(reread["result"], now, "重启以后照样是那一份");
}

#[tokio::test]
async fn over_64_kib_is_refused_and_nothing_changes() {
    let home = Home::new();
    let (_core, mut head) = with_pane(&home).await;
    // `{"k":"…"}` 比字多 8 个字节：正好到上限的收。
    let fits = "x".repeat(LIMIT - 8);
    let full = head
        .call(
            "s1",
            "prefs.set",
            json!({"package": "pane", "values": {"k": fits}}),
        )
        .await;
    assert_eq!(
        full["result"]["values"]["k"].as_str().map(str::len),
        Some(LIMIT - 8),
        "正好到上限的收"
    );

    let more = head
        .call(
            "s2",
            "prefs.set",
            json!({"package": "pane", "values": {"j": 1}}),
        )
        .await;
    assert_eq!(reason(&more), "prefs_too_big", "{more}");
    assert_eq!(
        more["error"]["data"],
        json!({"reason": "prefs_too_big", "size": LIMIT + 6, "limit": LIMIT}),
        "带量到的和上限"
    );
    let read = head
        .call("g", "prefs.get", json!({"package": "pane"}))
        .await;
    assert_eq!(
        read["result"]["values"]
            .as_object()
            .map(|values| values.len()),
        Some(1),
        "什么都没改"
    );
    let file =
        std::fs::read_to_string(home.root.package_state("pane").join("prefs.json")).expect("在");
    assert!(!file.contains("\"j\""), "文件也没改");
}

#[tokio::test]
async fn other_heads_hear_the_change_and_the_setter_does_not() {
    let home = Home::new();
    let (core, mut setter) = with_pane(&home).await;
    let mut other = Client::connect(Arc::clone(&core));
    other.hello().await;
    let mut elsewhere = Client::connect(Arc::clone(&core));
    elsewhere.hello().await;
    head_subscribes(&mut setter).await;
    head_subscribes(&mut other).await;

    let set = setter
        .call(
            "s1",
            "prefs.set",
            json!({"package": "pane", "values": {"theme": "dusk"}}),
        )
        .await;
    assert_eq!(set["result"], json!({"values": {"theme": "dusk"}}), "{set}");
    assert_eq!(
        changed(&mut other).await,
        json!({"package": "pane", "values": {"theme": "dusk"}}),
        "别的连接收到整份"
    );
    assert_eq!(
        heard(&mut setter).await,
        Vec::<Value>::new(),
        "改的那一个不收"
    );
    assert_eq!(
        heard(&mut elsewhere).await,
        Vec::<Value>::new(),
        "没订的不收"
    );

    let subscribed = elsewhere
        .call(
            "p",
            "subscribe",
            json!({"stream": "prefs", "package": "pane"}),
        )
        .await;
    assert_eq!(
        subscribed["result"],
        json!({"values": {"theme": "dusk"}}),
        "订阅的回应带这一刻的整份"
    );
    let gone = other
        .call(
            "u",
            "unsubscribe",
            json!({"stream": "prefs", "package": "pane"}),
        )
        .await;
    assert_eq!(gone["result"], json!({}), "{gone}");
    setter
        .call(
            "s2",
            "prefs.set",
            json!({"package": "pane", "values": {"theme": null}}),
        )
        .await;
    assert_eq!(
        changed(&mut elsewhere).await,
        json!({"package": "pane", "values": {}}),
        "删光了推空的"
    );
    assert_eq!(heard(&mut other).await, Vec::<Value>::new(), "取消了不再收");
}

/// 订 `pane` 的设置，回应是这一刻的整份（还没有的是空的）。
async fn head_subscribes(client: &mut Client) {
    let subscribed = client
        .call(
            "p",
            "subscribe",
            json!({"stream": "prefs", "package": "pane"}),
        )
        .await;
    assert_eq!(subscribed["result"], json!({"values": {}}), "{subscribed}");
}

#[tokio::test]
async fn only_installed_ui_packages_have_prefs() {
    let home = Home::new();
    let (_core, mut head) = with_pane(&home).await;
    for (package, wanted) in [
        ("nowhere", "unknown_package"),
        ("../pane", "unknown_package"),
        ("miyu-base-shell", "not_a_ui_package"),
    ] {
        let get = head
            .call("g", "prefs.get", json!({"package": package}))
            .await;
        assert_eq!(reason(&get), wanted, "{package}：{get}");
        let set = head
            .call(
                "s",
                "prefs.set",
                json!({"package": package, "values": {"a": 1}}),
            )
            .await;
        assert_eq!(reason(&set), wanted, "{package}：{set}");
        let subscribed = head
            .call(
                "p",
                "subscribe",
                json!({"stream": "prefs", "package": package}),
            )
            .await;
        assert_eq!(reason(&subscribed), wanted, "{package}：{subscribed}");
    }
    assert!(
        !home
            .root
            .package_state("miyu-base-shell")
            .join("prefs.json")
            .exists(),
        "不是界面包的什么都不写"
    );
    for params in [
        json!({}),
        json!({"package": "pane", "values": [1]}),
        json!({"package": "pane", "values": "x"}),
        json!({"package": "pane"}),
    ] {
        let set = head.call("s", "prefs.set", params.clone()).await;
        assert_eq!(reason(&set), "bad_params", "{params}：{set}");
    }
    let bare = head
        .call("p", "subscribe", json!({"stream": "prefs"}))
        .await;
    assert_eq!(reason(&bare), "bad_params", "订阅要写包：{bare}");
}

//! 管理员模式（施工 Y-12 下，设计 06 八再补第 2 条；2026-10-24 项目主人定本机也要切、账号管理也在管理员模式下）：握手声明了
//! 认识管理员模式（`caps.scope`）的连接，改系统那一层要带 `"scope": "system"`，不带的 `admin_mode_required`；没声明的老头照
//! 以前的意思；成员带了一律 `not_permitted`；`secret.set` 不写 `layer` 的照模式存；组里别人改的系统配置落在大家共用的那一份。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::login::*;
use crate::support::*;

const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

/// 本机令牌、声明了认识管理员模式的连接。
async fn scoped(core: &std::sync::Arc<miyu_endpoint::Core>) -> Client {
    let mut client = Client::connect(std::sync::Arc::clone(core));
    let reply = client
        .call(
            "h",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "locale": "zh-CN",
                "caps": {"input": true, "scope": true},
                "token": TOKEN,
            }),
        )
        .await;
    assert!(reply.get("result").is_some(), "{reply}");
    client
}

fn reason(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

fn read(home: &Home, relative: &str) -> String {
    std::fs::read_to_string(home.root.path().join(relative)).unwrap_or_default()
}

#[tokio::test]
async fn system_writes_need_admin_mode_when_the_head_knows_it() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = scoped(&core).await;
    let system_set = json!({"layer": "system", "changes": [{"key": "ui.language", "value": "en"}]});
    // 平时：改系统那一层的拒、改自己的照常、读的照常。
    for (method, params) in [
        ("config.set", system_set.clone()),
        (
            "secret.set",
            json!({"name": "shared", "value": FAKE, "layer": "system"}),
        ),
        ("account.invite", json!({})),
        ("account.list", json!({})),
        // 装回、卸出厂的包是系统那一层的事（施工 U23 下上：装进自己那一层的不要管理员模式）。
        ("package.install", json!({"package": "glob"})),
        ("package.remove", json!({"package": "glob"})),
        ("package.enable", json!({"package": "x"})),
        ("mcp.set", json!({})),
    ] {
        let reply = owner.call("x", method, params).await;
        assert_eq!(
            reason(&reply),
            Some("admin_mode_required"),
            "{method}: {reply}"
        );
    }
    assert!(read(&home, "system/config.toml").is_empty(), "什么都没写");
    let mine = owner
        .call(
            "p",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "ui.language", "value": "ja"}]}),
        )
        .await;
    assert!(mine.get("result").is_some(), "{mine}");
    let listed = owner.call("l", "package.list", json!({})).await;
    assert!(listed.get("result").is_some(), "{listed}");
    // 平时 secret.set 不写 layer 的存自己那一份。
    let personal = owner
        .call("s", "secret.set", json!({"name": "mine", "value": FAKE}))
        .await;
    assert!(personal.get("result").is_some(), "{personal}");
    assert!(read(&home, "home/alice/secrets.toml").contains("mine"));
    assert!(!read(&home, "system/secrets.toml").contains("mine"));
    // 管理员模式里：照常；不写 layer 的存系统那一份。
    let written = call_system(&mut owner, "y", "config.set", system_set).await;
    assert!(written.get("result").is_some(), "{written}");
    assert!(read(&home, "system/config.toml").contains("language"));
    let shared = call_system(
        &mut owner,
        "s2",
        "secret.set",
        json!({"name": "shared", "value": FAKE}),
    )
    .await;
    assert!(shared.get("result").is_some(), "{shared}");
    assert!(read(&home, "system/secrets.toml").contains("shared"));
    let invited = call_system(&mut owner, "i", "account.invite", json!({})).await;
    assert!(invited.get("result").is_some(), "{invited}");
}

#[tokio::test]
async fn old_heads_keep_the_old_meaning() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let written = owner
        .call(
            "y",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "ui.language", "value": "en"}]}),
        )
        .await;
    assert!(written.get("result").is_some(), "{written}");
    let shared = owner
        .call("s", "secret.set", json!({"name": "shared", "value": FAKE}))
        .await;
    assert!(shared.get("result").is_some(), "{shared}");
    assert!(
        read(&home, "system/secrets.toml").contains("shared"),
        "老头的管理员照旧存系统那一份"
    );
}

#[tokio::test]
async fn members_and_bad_scopes_are_refused() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut bob = member(&core, "bob").await;
    for (method, params) in [
        ("session.list", json!({})),
        ("secret.set", json!({"name": "x", "value": FAKE})),
    ] {
        let reply = call_system(&mut bob, "x", method, params).await;
        assert_eq!(reason(&reply), Some("not_permitted"), "{method}: {reply}");
    }
    assert!(
        read(&home, "home/bob/secrets.toml").is_empty(),
        "什么都没写"
    );
    let mut owner = scoped(&core).await;
    owner
        .line(r#"{"jsonrpc":"2.0","id":"b","method":"session.list","params":{},"scope":"root"}"#)
        .await;
    let reply = owner.next().await.expect("有回应");
    assert_eq!(reply["error"]["code"], -32600, "{reply}");
}

/// 组里别的管理员在管理员模式里改的系统配置，落在大家共用的那一份：机主那边跟着换。
#[tokio::test]
async fn another_admin_writes_the_shared_system_layer() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let mut bob = member(&core, "bob").await;
    let made = call_system(
        &mut owner,
        "a",
        "account.set_admin",
        json!({"account": "bob", "admin": true}),
    )
    .await;
    assert_eq!(made["result"], json!({}), "{made}");
    let written = call_system(
        &mut bob,
        "y",
        "config.set",
        json!({"layer": "system", "changes": [{"key": "terminal.idle_new_after", "value": 9}]}),
    )
    .await;
    assert!(written.get("result").is_some(), "{written}");
    let got = owner
        .call(
            "g",
            "config.get",
            json!({"keys": ["terminal.idle_new_after"]}),
        )
        .await;
    assert_eq!(
        got["result"]["items"]["terminal.idle_new_after"]["value"], 9,
        "{got}"
    );
    let shared = call_system(
        &mut bob,
        "s",
        "secret.set",
        json!({"name": "shared", "value": FAKE}),
    )
    .await;
    assert!(shared.get("result").is_some(), "{shared}");
    assert!(read(&home, "system/secrets.toml").contains("shared"));
    let listed = owner.call("l", "secret.list", json!({})).await;
    assert!(
        listed.to_string().contains("shared"),
        "机主那一份看得到：{listed}"
    );
    // 组里的管理员订得了软件包、扩展的流（列得出就该看得到它变）。
    for stream in ["packages", "extensions"] {
        let subscribed = bob.call("s", "subscribe", json!({"stream": stream})).await;
        assert!(subscribed.get("result").is_some(), "{stream}: {subscribed}");
    }
}

/// 在管理员模式里的头重连，握手那一条也带着 `scope`：照常握手。
#[tokio::test]
async fn a_hello_with_scope_still_shakes_hands() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(core);
    let reply = call_system(
        &mut client,
        "h",
        "hello",
        json!({
            "protocol": [1, 1],
            "head": {"kind": "test", "version": "0.0.0"},
            "caps": {"scope": true},
            "token": TOKEN,
        }),
    )
    .await;
    assert!(reply.get("result").is_some(), "{reply}");
}

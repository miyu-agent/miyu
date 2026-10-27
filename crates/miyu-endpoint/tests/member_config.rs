//! 每人一份配置（施工 Y-2 下下中上，设计 06 U21；2026-10-24 项目主人定「成员只继承系统层」）：成员读得到系统配置、读不到机主的
//! 个人设置；机主改了系统配置，已经开着的成员那一份跟着换；成员改开关写进他自己的个人设置，碰不到机主的；握手说是不是管理员。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::login::*;
use crate::support::*;

/// `config.get` 交回的一项的值。
async fn value(client: &mut Client, key: &str) -> Value {
    let reply = client.call("g", "config.get", json!({"keys": [key]})).await;
    reply["result"]["items"][key]["value"].clone()
}

async fn set(client: &mut Client, layer: &str, key: &str, value: Value) -> Value {
    client
        .call(
            "s",
            "config.set",
            json!({"layer": layer, "changes": [{"key": key, "value": value}]}),
        )
        .await
}

#[tokio::test]
async fn a_member_reads_the_system_layer_and_keeps_his_own_settings() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let mut bob = member(&core, "bob").await;

    // 机主的个人设置成员读不到。
    let mine = set(&mut owner, "personal", "ui.language", json!("ja")).await;
    assert!(mine.get("result").is_some(), "{mine}");
    assert_eq!(value(&mut owner, "ui.language").await, json!("ja"));
    assert_ne!(value(&mut bob, "ui.language").await, json!("ja"));

    // 系统配置成员读得到；改了以后，已经开着的那一份跟着换。
    let before = value(&mut bob, "terminal.idle_new_after").await;
    let system = set(&mut owner, "system", "terminal.idle_new_after", json!(7)).await;
    assert!(system.get("result").is_some(), "{system}");
    assert_ne!(before, json!(7));
    assert_eq!(value(&mut bob, "terminal.idle_new_after").await, json!(7));

    // 成员改开关写进他自己的个人设置。
    let switched = bob
        .call(
            "f",
            "feature.set",
            json!({"feature": "skill-notes", "on": false}),
        )
        .await;
    assert_eq!(switched["result"], json!({}), "{switched}");
    let his = std::fs::read_to_string(home.root.account_dir(&account("bob")).join("settings.toml"))
        .expect("写进了他的个人设置");
    assert!(his.contains("skill-notes"), "{his}");
    let owners = std::fs::read_to_string(home.root.account_dir(&alice()).join("settings.toml"))
        .unwrap_or_default();
    assert!(!owners.contains("skill-notes"), "{owners}");

    // 写配置（施工 Y-2 下下中中）：他的个人设置写得了，碰不到机主的；系统配置写不了。
    let written = set(&mut bob, "personal", "ui.language", json!("en")).await;
    assert!(written.get("result").is_some(), "{written}");
    assert_eq!(value(&mut bob, "ui.language").await, json!("en"));
    assert_eq!(value(&mut owner, "ui.language").await, json!("ja"));
    let refused = set(&mut bob, "system", "terminal.idle_new_after", json!(9)).await;
    assert_eq!(reason(&refused), Some("not_permitted"), "{refused}");
    assert_eq!(value(&mut bob, "terminal.idle_new_after").await, json!(7));
}

#[tokio::test]
async fn the_handshake_says_who_is_an_admin() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = Client::connect(std::sync::Arc::clone(&core));
    let said = owner.hello().await;
    assert_eq!(said["result"]["admin"], json!(true), "{said}");
    // 成员登录进来的那一次握手（[`member`] 里）说不是；这里另开一个连接照他的用户名密码再握一次手看。
    let _bob = member(&core, "bob").await;
    let mut again = Client::connect(std::sync::Arc::clone(&core));
    let said = hello_with(
        &mut again,
        json!({"user": "bob", "password": "correct horse"}),
    )
    .await;
    assert_eq!(said["result"]["admin"], json!(false), "{said}");
}

fn account(name: &str) -> miyu_kernel::id::AccountId {
    miyu_kernel::id::AccountId::parse(name).expect("合写法")
}

/// 成员的会话照他那一份配置（施工 Y-2 下下中上）：他把 `read` 这个包对自己关了，他开的会话工具面上没有 `read`；机主的会话照旧
/// 有（施工 FL-1 起工具一律全量，原来拿「改成按需加载」当记号，换成这一项）。
#[tokio::test]
async fn a_members_session_runs_on_his_own_settings() {
    use miyu_session::testkit::Play;
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let core = crate::scripts::core(&home, &script);
    let mut owner = local(&core).await;
    let mut bob = member(&core, "bob").await;
    let set = set(&mut bob, "personal", "packages.read.off", json!(true)).await;
    assert!(set.get("result").is_some(), "{set}");
    let names = |n: usize| -> Vec<String> {
        script.requests()[n]
            .1
            .tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect()
    };
    let his = bob
        .call("c", "session.create", json!({"preset": "full"}))
        .await["result"]["session"]
        .as_str()
        .expect("造得出")
        .to_string();
    bob.say("s", &his, "hi").await;
    until("他的那一轮发出去", || !script.requests().is_empty()).await;
    assert!(
        !names(0).iter().any(|name| name == "read"),
        "{:?}",
        names(0)
    );
    let cwd = home.work.to_string_lossy().into_owned();
    let mine = owner.create("o", &cwd).await;
    owner.say("o2", &mine, "hi").await;
    until("机主的那一轮发出去", || {
        script.requests().len() > 1
    })
    .await;
    assert!(names(1).iter().any(|name| name == "read"), "{:?}", names(1));
}

/// 给人看的字照这个人那一份的 `ui.language`（施工 Y-2 下下末）：bob 设成英文，他的连接说英文；机主照旧中文。
#[tokio::test]
async fn each_person_hears_his_own_language() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let mut bob = member(&core, "bob").await;
    let set = set(&mut bob, "personal", "ui.language", json!("en")).await;
    assert!(set.get("result").is_some(), "{set}");
    let refused = bob.call("r", "fs.read", json!({"path": "/"})).await;
    assert_eq!(refused["error"]["message"], "Not permitted.", "{refused}");
    let refused = owner
        .call("r", "account.disable", json!({"account": "nobody"}))
        .await;
    assert!(
        refused["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_ascii()),
        "机主照旧中文：{refused}"
    );
    let mut again = member(&core, "carol").await;
    let hello = again
        .call("g", "config.get", json!({"keys": ["ui.language"]}))
        .await;
    assert!(hello.get("result").is_some(), "{hello}");
}

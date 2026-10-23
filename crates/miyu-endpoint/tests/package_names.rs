//! 包编号和人的用户名互相占位（施工 Y-5 中下，Y-5 施工单第五节第 1 条）：每个带进程的包是自己的账号，账号名就是包的编号，人
//! 不能注册成已装的包、本机那份官方索引里的包的编号；包也不能装成已经有人用着的名字（`package_id_taken`）。同一个包再装一次
//! （升级）照常。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;

use crate::support::login::*;
use crate::support::source::{keys, signed};
use crate::support::*;

/// 一个带进程的包的清单。
fn manifest(id: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"miyu-xtool\"\nabout = {{ en = \"X\" }}\n\n[process]\n"
    )
}

/// 一个吉祥物包：不带进程、没有自己的账号和家目录，编号照样算占着。
fn mascot(home: &Home, id: &str) -> String {
    let dir = home.work.join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(
        dir.join("package.toml"),
        format!("[package]\nname = {{ en = \"{id}\" }}\n\n[mascot]\nmodel = \"m.json\"\n"),
    )
    .expect("写得进");
    std::fs::write(dir.join("m.json"), "{}").expect("写得进");
    dir.to_str().expect("UTF-8").to_string()
}

/// 在工作目录里放一个包文件夹，交回路径。
fn folder(home: &Home, id: &str) -> String {
    let dir = home.work.join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("package.toml"), manifest(id)).expect("写得进");
    dir.to_str().expect("UTF-8").to_string()
}

/// 管理员发一个邀请码，用它注册 `username`，交回回应。
async fn register(core: &Arc<Core>, username: &str) -> Value {
    let mut admin = local(core).await;
    let code = admin.call("i", "account.invite", json!({})).await["result"]["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let mut client = Client::connect(Arc::clone(core));
    hello_with(&mut client, json!({"invite": code})).await;
    client
        .call(
            "r",
            "account.register",
            json!({"username": username, "password": "correct horse"}),
        )
        .await
}

#[tokio::test]
async fn people_cannot_take_an_installed_or_indexed_package_id() {
    let home = Home::new();
    let (secret, public) = keys();
    let core = crate::support::source::core(&home, "http://127.0.0.1:9/", &[], public);
    let (index, signature) = signed("2026-10-23T00:00:00Z", &["miyu-render"], &secret);
    let sync = home.root.state().join("packages/.sync");
    std::fs::create_dir_all(&sync).expect("建得了");
    std::fs::write(sync.join("index.json"), index).expect("写得进");
    std::fs::write(sync.join("index.json.sig"), signature).expect("写得进");
    let mut admin = local(&core).await;
    let installed = admin
        .call(
            "p",
            "package.install",
            json!({"path": mascot(&home, "kit")}),
        )
        .await;
    assert!(installed.get("error").is_none(), "{installed}");
    assert!(
        !home.root.path().join("home/kit").exists(),
        "不带进程的包没有家目录：占着靠编号"
    );
    for name in ["kit", "miyu-render"] {
        let refused = register(&core, name).await;
        assert_eq!(why(&refused), "username_taken", "{name}: {refused}");
    }
    let made = register(&core, "bob").await;
    assert_eq!(made["result"]["account"], "bob", "{made}");
}

#[tokio::test]
async fn a_package_cannot_take_a_persons_name_but_upgrades_itself() {
    let home = Home::new();
    let core = home.core(&miyu_session::testkit::Script::new([]));
    let made = register(&core, "bob").await;
    assert_eq!(made["result"]["account"], "bob", "{made}");
    let mut admin = local(&core).await;
    let refused = admin
        .call(
            "p1",
            "package.install",
            json!({"path": folder(&home, "bob")}),
        )
        .await;
    assert_eq!(why(&refused), "package_id_taken", "{refused}");
    for id in ["p2", "p3"] {
        let installed = admin
            .call(id, "package.install", json!({"path": folder(&home, "kit")}))
            .await;
        assert!(
            installed.get("error").is_none(),
            "装了再装是升级：{installed}"
        );
    }
}

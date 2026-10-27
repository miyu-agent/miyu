//! 每人一份的扩展在软件包列表里的状态（施工 U23 下上，网页报的）：照看的人自己那一份报，不照系统那一份（那一份不存在，恒是关着）。
//! 起过的照它这时的样子，还没起过的报 `ready`：用到时才起，不是关着。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::login::member;
use crate::support::*;

/// `client` 列出来的包 `id` 的状态。
async fn status(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["package"] == id)
        .map(|one| one["status"].clone())
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

#[tokio::test]
async fn a_per_account_extension_shows_the_viewer_s_own_copy() {
    let home = Home::new();
    let program = Program::new();
    let tools = json!({"tools": [{"name": "ping", "description": "Ping.",
        "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]});
    install_providing(
        &home,
        "each",
        &program.name(),
        "always",
        &steps(&[
            "record:calls.log",
            "hello",
            &format!("ask:provide:{tools}"),
            "serve",
        ]),
    );
    let manifest = home
        .root
        .system_packages()
        .join("each")
        .join("package.toml");
    let text = read(&manifest)
        .replacen("[package]\n", "[package]\nmembers = true\n", 1)
        .replacen("[process]\n", "[process]\nscope = \"account\"\n", 1);
    std::fs::write(&manifest, text).expect("写得进");
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    // 头一回装上、没有登记缓存：先替机主起一份。机主看到的是他那一份在跑。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while status(&mut owner, "each").await != "running" {
        assert!(
            tokio::time::Instant::now() < deadline,
            "机主那一份一直没跑起来"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // 成员那一份还没起过：用到时才起，报 ready，不是关着。
    let mut bob = member(&core, "bob").await;
    assert_eq!(status(&mut bob, "each").await, "ready");
    let theirs = home
        .root
        .account_dir(&AccountId::parse("bob").expect("合写法"))
        .join("state/packages/each");
    assert!(!theirs.exists(), "看一眼不替他起");
}

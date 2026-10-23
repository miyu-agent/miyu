//! 设置那一段（施工 Y-12 中上，设计 06 U22）：还没有机主的核心，本机令牌握手说要设置、不是谁，别的方法 `setup_first`；注册成了
//! 机主，这一段的别的连接一起断开，交回的那一声响了；远程的网页拿一次性码也注册得成；已经有机主了再来的拒。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_store::accounts;

use crate::support::login::hello_with;
use crate::support::*;

const PASSWORD: &str = "correct horse battery";

/// 设置那一段的核心。
fn setting_up(home: &Home) -> Arc<Core> {
    Arc::new(
        home.core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
            .in_setup(),
    )
}

/// 账号文件里的机主。
fn owner(home: &Home) -> Option<String> {
    accounts::read(&home.root.system().join(accounts::FILE))
        .expect("读得出")
        .accounts
        .owner()
        .map(|owner| owner.id.clone())
}

#[tokio::test]
async fn registering_the_owner_closes_this_stage_and_says_so() {
    let home = Home::new();
    let core = setting_up(&home);
    let done = core.setup_done().expect("在设置那一段");
    let mut watching = Client::connect(Arc::clone(&core));
    let reply = watching.hello().await;
    assert_eq!(reply["result"]["setup"], json!(true), "{reply}");
    assert_eq!(reply["result"]["account"], Value::Null, "{reply}");
    let mut setting = Client::connect(Arc::clone(&core));
    setting.hello().await;
    let refused = setting.call("p", "persona.list", json!({})).await;
    assert_eq!(
        refused["error"]["data"]["reason"], "setup_first",
        "{refused}"
    );
    let set = setting
        .call(
            "s",
            "account.setup",
            json!({"username": "shorin", "password": PASSWORD}),
        )
        .await;
    assert_eq!(set["result"]["account"], "shorin", "{set}");
    tokio::time::timeout(Duration::from_secs(5), done.notified())
        .await
        .expect("响了");
    assert!(setting.next().await.is_none(), "答完就断");
    assert!(watching.next().await.is_none(), "这一段别的连接一起断");
    assert_eq!(owner(&home).as_deref(), Some("shorin"));
    assert!(
        home.root
            .account_dir(&miyu_kernel::id::AccountId::parse("shorin").expect("合写法"))
            .is_dir()
    );
}

#[tokio::test]
async fn a_remote_page_registers_the_owner_with_a_code_and_only_once() {
    let home = Home::new();
    let core = setting_up(&home);
    let mut terminal = Client::connect(Arc::clone(&core));
    terminal.hello().await;
    let code = terminal.call("c", "account.setup_code", json!({})).await["result"]["code"]
        .as_str()
        .expect("设置那一段也给码")
        .to_string();
    let mut browser = Client::connect(Arc::clone(&core));
    hello_with(&mut browser, json!({"code": code})).await;
    let set = browser
        .call(
            "s",
            "account.setup",
            json!({"username": "shorin", "password": PASSWORD}),
        )
        .await;
    assert_eq!(set["result"]["account"], "shorin", "{set}");
    // 晚到的（两个头同时设置）：已经有机主了。
    let late = Arc::new(
        home.core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
            .in_setup(),
    );
    let mut second = Client::connect(late);
    second.hello().await;
    let refused = second
        .call(
            "s2",
            "account.setup",
            json!({"username": "bob", "password": PASSWORD}),
        )
        .await;
    assert_eq!(
        refused["error"]["data"]["reason"], "not_permitted",
        "{refused}"
    );
    assert_eq!(owner(&home).as_deref(), Some("shorin"), "只有一个机主");
}

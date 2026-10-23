//! 管理员组（施工 Y-12 中下，设计 06 U16、U19 第 4 条）：机主恒是管理员、撤不掉；机主能把成员设成管理员，设了的能管账号（发邀请、
//! 列账号、停用启用、设别人），撤了就不能；不是管理员的调 `account.set_admin` 拒；对机主、系统、包的账号、没有的账号回
//! `unknown_account`，停用的成员回 `member_disabled`；`account.list` 每个人带 `admin`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::Script;

use crate::support::login::*;
use crate::support::*;

/// 机主发一个邀请码，用它注册 `username`，交回这个人登录进来的连接。
async fn member(core: &Arc<Core>, username: &str) -> Client {
    let mut owner = local(core).await;
    let code = owner.call("i", "account.invite", json!({})).await["result"]["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let mut client = Client::connect(Arc::clone(core));
    hello_with(&mut client, json!({"invite": code})).await;
    let made = client
        .call(
            "r",
            "account.register",
            json!({"username": username, "password": "correct horse"}),
        )
        .await;
    let token = made["result"]["login"]["token"]
        .as_str()
        .unwrap_or_else(|| panic!("{made}"))
        .to_string();
    let mut back = Client::connect(Arc::clone(core));
    hello_with(&mut back, json!({"login": token})).await;
    back
}

/// `account.list` 里 `account` 那一行的 `admin`。
async fn listed_admin(client: &mut Client, account: &str) -> Value {
    let reply = client.call("l", "account.list", json!({})).await;
    reply["result"]["accounts"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|row| row["account"] == account)
        .map(|row| row["admin"].clone())
        .unwrap_or_else(|| panic!("{account} 不在：{reply}"))
}

async fn set(client: &mut Client, account: &str, admin: bool) -> Value {
    client
        .call(
            "s",
            "account.set_admin",
            json!({"account": account, "admin": admin}),
        )
        .await
}

#[tokio::test]
async fn the_owner_makes_and_unmakes_admins() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let mut bob = member(&core, "bob").await;
    assert_eq!(
        listed_admin(&mut owner, "alice").await,
        true,
        "机主恒是管理员"
    );
    assert_eq!(listed_admin(&mut owner, "bob").await, false);
    let refused = bob.call("i", "account.invite", json!({})).await;
    assert_eq!(why(&refused), "not_permitted", "{refused}");
    assert_eq!(why(&set(&mut bob, "bob", true).await), "not_permitted");

    let reply = set(&mut owner, "bob", true).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(listed_admin(&mut owner, "bob").await, true);
    let invited = bob.call("i", "account.invite", json!({})).await;
    assert!(invited.get("error").is_none(), "设了的能发邀请：{invited}");
    assert_eq!(listed_admin(&mut bob, "bob").await, true, "设了的能列账号");
    let refused = bob.call("c", "config.get", json!({})).await;
    assert_eq!(
        why(&refused),
        "not_permitted",
        "账号管理以外的照旧不放（Y-2 下下）：{refused}"
    );

    assert_eq!(set(&mut owner, "bob", false).await["result"], json!({}));
    let refused = bob.call("i", "account.invite", json!({})).await;
    assert_eq!(why(&refused), "not_permitted", "撤了就不能：{refused}");
}

#[tokio::test]
async fn the_owner_system_and_strangers_cannot_be_set() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let _carol = member(&core, "carol").await;
    // 机主设好用户名、密码以后 `accounts.json` 里有他那一行：照样不能设（不靠「表里没有」挡）。
    set_up(&core, "shorin", "correct horse").await;
    for account in ["alice", "admin", "ghost", "Not An Id"] {
        let refused = set(&mut owner, account, false).await;
        assert_eq!(why(&refused), "unknown_account", "{account}: {refused}");
    }
    assert_eq!(listed_admin(&mut owner, "alice").await, true, "机主撤不掉");
    let disabled = owner
        .call("d", "account.disable", json!({"account": "carol"}))
        .await;
    assert!(disabled.get("error").is_none(), "{disabled}");
    let refused = set(&mut owner, "carol", true).await;
    assert_eq!(why(&refused), "member_disabled", "{refused}");
    let refused = owner
        .call("s", "account.set_admin", json!({"account": "carol"}))
        .await;
    assert_eq!(why(&refused), "bad_params", "{refused}");
}

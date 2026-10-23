//! 成员（施工 Y-1 上，设计 06 第二版）：管理员发、列、撤邀请码；用邀请码握手只能注册；注册成了换出能用的登录令牌，用户名
//! 密码也登得进；重名、保留名、用过的、撤掉的码拒，用户名占了的不用掉码；停用以后进不来、连着的断开，启用以后又能进；成员的
//! 连接在 Y-2 以前只能管自己的账号；管理员列得出账号。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_store::invites;

use crate::support::login::*;
use crate::support::*;

/// 管理员发一个邀请码，交回 `result`。
async fn invite(core: &Arc<miyu_endpoint::Core>) -> Value {
    let mut admin = local(core).await;
    admin.call("i", "account.invite", json!({})).await["result"].clone()
}

/// 用邀请码 `code` 连上的一个客户端。
async fn invited(core: &Arc<miyu_endpoint::Core>, code: &str) -> Client {
    let mut client = Client::connect(Arc::clone(core));
    let reply = hello_with(&mut client, json!({"invite": code})).await;
    assert_eq!(reply["result"]["register"], true, "{reply}");
    assert_eq!(reply["result"]["account"], Value::Null, "还没注册，不是谁");
    client
}

/// 用 `code` 注册 `username`、密码 `correct horse`，交回登录令牌。
async fn register(core: &Arc<miyu_endpoint::Core>, code: &str, username: &str) -> String {
    let mut client = invited(core, code).await;
    let reply = client
        .call(
            "r",
            "account.register",
            json!({"username": username, "password": "correct horse"}),
        )
        .await;
    assert_eq!(reply["result"]["account"], username, "{reply}");
    reply["result"]["login"]["token"]
        .as_str()
        .expect("{reply}")
        .to_string()
}

fn bob() -> AccountId {
    AccountId::parse("bob").expect("合写法")
}

#[tokio::test]
async fn the_admin_issues_lists_and_revokes_invites() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut admin = local(&core).await;
    for days in [0, 31] {
        let refused = admin
            .call("d", "account.invite", json!({"days": days}))
            .await;
        assert_eq!(why(&refused), "bad_params", "{days}: {refused}");
    }
    let issued = admin.call("i", "account.invite", json!({"days": 3})).await["result"].clone();
    let code = issued["code"].as_str().expect("有码");
    let id = issued["id"].as_str().expect("有编号");
    assert_eq!(code.len(), 32, "{issued}");
    assert_eq!(id.len(), 8, "{issued}");
    let text = std::fs::read_to_string(home.root.system().join(invites::FILE)).expect("写了");
    assert!(!text.contains(code), "只存哈希：{text}");
    let listed = admin.call("l", "account.invites", json!({})).await;
    let rows = listed["result"]["invites"].as_array().expect("{listed}");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], id);
    assert!(rows[0].get("code").is_none(), "列出来不带码");
    let revoked = admin
        .call("v", "account.invite_revoke", json!({"id": id}))
        .await;
    assert_eq!(revoked["result"]["revoked"], true, "{revoked}");
    let again = admin
        .call("v", "account.invite_revoke", json!({"id": id}))
        .await;
    assert_eq!(why(&again), "unknown_invite", "{again}");
    let mut stranger = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut stranger, json!({"invite": code})).await;
    assert_eq!(why(&refused), "bad_invite", "撤掉的码进不来：{refused}");
}

#[tokio::test]
async fn an_invite_registers_a_member_once() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let code = invite(&core).await["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let mut client = invited(&core, &code).await;
    let refused = client.call("l", "session.list", json!({})).await;
    assert_eq!(why(&refused), "register_first", "{refused}");
    for (username, reason) in [
        ("alice", "username_taken"),
        // `admin` 是系统（施工 Y-12 中上）：机主是别人的时候照样注册不成。
        ("admin", "username_taken"),
        ("users", "username_taken"),
        ("Bob", "bad_params"),
    ] {
        let refused = client
            .call(
                "r",
                "account.register",
                json!({"username": username, "password": "correct horse"}),
            )
            .await;
        assert_eq!(why(&refused), reason, "{username}: {refused}");
    }
    let refused = client
        .call(
            "r",
            "account.register",
            json!({"username": "bob", "password": "short"}),
        )
        .await;
    assert_eq!(why(&refused), "bad_params", "{refused}");
    // 用户名占了、写错了的都没用掉码：同一个连接照样注册得成。
    let reply = client
        .call(
            "r",
            "account.register",
            json!({"username": "bob", "password": "correct horse"}),
        )
        .await;
    assert_eq!(reply["result"]["account"], "bob", "{reply}");
    let token = reply["result"]["login"]["token"].as_str().expect("{reply}");
    assert!(home.root.workspace(&bob()).is_dir(), "家目录、工作区建好了");
    // 码用掉了。
    let mut late = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut late, json!({"invite": code})).await;
    assert_eq!(why(&refused), "bad_invite", "{refused}");
    // 登录令牌、用户名密码都登得进，是 bob。
    let mut back = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut back, json!({"login": token})).await;
    assert_eq!(reply["result"]["account"], "bob", "{reply}");
    let mut typed = Client::connect(Arc::clone(&core));
    let reply = hello_with(
        &mut typed,
        json!({"user": "bob", "password": "correct horse"}),
    )
    .await;
    assert_eq!(reply["result"]["account"], "bob", "{reply}");
    // Y-2 以前成员只能管自己的账号。抓链接卡片也拒（施工 MB-2：她替成员上网不碰本机和局域网，Y-2 放开时照连接的账号
    // 分机器主人和别人）。
    for method in [
        "session.list",
        "account.invite",
        "account.list",
        "config.get",
        "link.preview",
    ] {
        let refused = back.call("m", method, json!({})).await;
        assert_eq!(why(&refused), "not_permitted", "{method}: {refused}");
    }
    // 管理员列得出来。
    let mut admin = local(&core).await;
    let listed = admin.call("a", "account.list", json!({})).await;
    let rows = listed["result"]["accounts"].as_array().expect("{listed}");
    assert_eq!(rows[0]["account"], "alice");
    assert_eq!(rows[0]["kind"], "admin");
    let bob_row = rows
        .iter()
        .find(|row| row["account"] == "bob")
        .expect("有 bob");
    assert_eq!(bob_row["kind"], "member");
    assert!(bob_row["created"].is_string(), "{bob_row}");
    assert!(bob_row.get("disabled").is_none());
}

#[tokio::test]
async fn disabling_cuts_a_member_off_and_enabling_lets_them_back() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let code = invite(&core).await["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let token = register(&core, &code, "bob").await;
    let mut connected = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut connected, json!({"login": token})).await;
    assert_eq!(reply["result"]["account"], "bob", "{reply}");
    let mut admin = local(&core).await;
    // 管理员也用登录令牌连着一条：停用别人只断那个人的连接。
    let admin_token = set_up(&core, "shorin", "correct horse").await;
    let mut admin_web = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut admin_web, json!({"login": admin_token})).await;
    assert_eq!(reply["result"]["account"], "alice", "{reply}");
    for target in ["alice", "nobody"] {
        let refused = admin
            .call("d", "account.disable", json!({"account": target}))
            .await;
        assert_eq!(why(&refused), "unknown_account", "{target}: {refused}");
    }
    let off = admin
        .call("d", "account.disable", json!({"account": "bob"}))
        .await;
    assert_eq!(off["result"]["disabled"], true, "{off}");
    assert!(is_closed(&mut connected).await, "连着的断开");
    let still = admin_web.call("s", "session.list", json!({})).await;
    assert!(still.get("result").is_some(), "管理员的连接还在：{still}");
    let mut again = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut again, json!({"login": token})).await;
    assert_eq!(why(&refused), "bad_login", "令牌作废了：{refused}");
    let mut typed = Client::connect(Arc::clone(&core));
    let refused = hello_with(
        &mut typed,
        json!({"user": "bob", "password": "correct horse"}),
    )
    .await;
    assert_eq!(
        why(&refused),
        "account_disabled",
        "密码对了才说停用了：{refused}"
    );
    assert!(is_closed(&mut typed).await, "回完断开");
    let mut wrong = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut wrong, json!({"user": "bob", "password": "wrong"})).await;
    assert_eq!(
        why(&refused),
        "bad_password",
        "密码不对的不透露停没停：{refused}"
    );
    let listed = admin.call("a", "account.list", json!({})).await;
    let rows = listed["result"]["accounts"].as_array().expect("{listed}");
    let bob_row = rows
        .iter()
        .find(|row| row["account"] == "bob")
        .expect("有 bob");
    assert_eq!(bob_row["disabled"], true);
    let on = admin
        .call("e", "account.enable", json!({"account": "bob"}))
        .await;
    assert_eq!(on["result"]["disabled"], false, "{on}");
    let mut back = Client::connect(Arc::clone(&core));
    let reply = hello_with(
        &mut back,
        json!({"user": "bob", "password": "correct horse"}),
    )
    .await;
    assert_eq!(reply["result"]["account"], "bob", "启用以后又能进：{reply}");
    // 管理员照旧什么都能调。
    let listed = admin.call("s", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "{listed}");
}

/// 握手的 `host.dirs`（施工 Y-11 上）：管理员的有 pictures、videos、documents、downloads 四处，照原样给、不建；成员的没有
/// （`06-多用户与身份.md` U25：普通用户看不到运行它的那台电脑，真实路径就是那台电脑的东西）。
#[tokio::test]
async fn only_the_admin_is_told_where_the_user_dirs_are() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut admin = Client::connect(Arc::clone(&core));
    let reply = admin.hello().await;
    let dirs = &reply["result"]["host"]["dirs"];
    let at = |name: &str| {
        home.root
            .account_dir(&alice())
            .join(name)
            .display()
            .to_string()
    };
    assert_eq!(
        dirs,
        &json!({
            "pictures": at("pictures"),
            "videos": at("videos"),
            "documents": at("documents"),
            "downloads": at("downloads"),
        }),
        "{reply}"
    );
    assert!(
        !home.root.account_dir(&alice()).join("pictures").exists(),
        "握手不建"
    );
    let code = invite(&core).await["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let token = register(&core, &code, "bob").await;
    let mut member = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut member, json!({"login": token})).await;
    assert_eq!(reply["result"]["account"], "bob", "{reply}");
    assert!(
        reply["result"]["host"].get("dirs").is_none(),
        "成员的没有：{reply}"
    );
}

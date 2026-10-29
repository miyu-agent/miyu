//! 身份（施工 W-8，`docs/blueprint/web-module.md`「怎么走」第一条；施工 SU-1 起只有一个主人）：一次性码只给出示本机令牌的连接、
//! 5 分钟、一次；用码连上的只能设访问密码，设好了换出设备令牌、落了盘才回、以前的设备全部作废；本机令牌连上的也能设、改；访问
//! 密码登录对的进、错的一样的话，照来源 60 秒错 5 次就先拒；没设访问密码的不认；网页软件转来的远程连接什么都没写、又没设密码的回
//! `password_not_set`；设备令牌认得、过期、作废的不认；凭据只写一种；`account.logout` 作废一个、全部，断开用设备令牌、密码连着的
//! 连接；文件坏了的样子。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_session::testkit::Script;
use miyu_store::{access, logins};

use crate::support::login::*;
use crate::support::*;

/// 读 `access.json`。
fn access_of(home: &Home) -> access::Access {
    access::read(&home.root.path().join(access::FILE))
        .expect("读得了")
        .access
}

/// 读主人的 `logins.json`。
fn logins_of(home: &Home) -> logins::Logins {
    logins::read(&home.root.path().join(logins::FILE))
        .expect("读得了")
        .logins
}

#[tokio::test]
async fn a_code_comes_only_from_the_local_token_and_only_sets_the_password() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let issued = setup_code(&core).await;
    let code = issued["code"].as_str().expect("有码");
    assert!(is_hex64(code), "{issued}");
    assert_eq!(issued["first"], true, "还没设过密码");
    assert!(issued["expires"].is_string(), "{issued}");
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"code": code})).await;
    assert_eq!(reply["result"]["setup"], true, "{reply}");
    assert!(
        reply["result"].get("account").is_none(),
        "没有账号这一格了：{reply}"
    );
    assert!(reply["result"].get("admin").is_none(), "{reply}");
    assert!(reply["result"].get("login").is_none());
    // 设好以前只能设密码；给人看的字照给。
    let refused = browser.call("l", "session.list", json!({})).await;
    assert_eq!(why(&refused), "setup_first", "{refused}");
    let human = browser.call("t", "human.get", json!({})).await;
    assert!(human.get("result").is_some(), "{human}");
    let refused = browser.call("c", "account.setup_code", json!({})).await;
    assert_eq!(why(&refused), "setup_first", "{refused}");
    // 写法不对的照旧是设密码的样子，可以再来；用户名那一格不认了。
    for params in [
        json!({"password": "short"}),
        json!({"password": "        "}),
        json!({"password": "x".repeat(1025)}),
        json!({"username": "shorin", "password": "correct horse"}),
    ] {
        let refused = browser.call("s", "account.setup", params.clone()).await;
        assert_eq!(why(&refused), "bad_params", "{params}: {refused}");
    }
    assert!(access_of(&home).password.is_empty(), "什么都没写");
    let reply = browser
        .call("s", "account.setup", json!({"password": "correct horse"}))
        .await;
    let token = reply["result"]["login"]["token"].as_str().expect("有令牌");
    assert!(is_hex64(token), "{reply}");
    assert!(reply["result"]["login"]["expires"].is_string());
    // 落了盘才回。
    assert!(access::verify(&access_of(&home).password, "correct horse"));
    assert!(logins_of(&home).valid(
        &logins::digest(token),
        miyu_kernel::time::Timestamp::from_unix_millis(0).expect("合")
    ));
    // 设好了，这个连接什么都调得了；设备令牌连上的改密码要带现在的（施工 SU-1 补，见下面）。
    let listed = browser.call("l", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "{listed}");
    let refused = browser
        .call("s", "account.setup", json!({"password": "another one"}))
        .await;
    assert_eq!(why(&refused), "bad_params", "{refused}");
    let refused = browser.call("c", "account.setup_code", json!({})).await;
    assert_eq!(why(&refused), "local_only", "{refused}");
    assert_eq!(setup_code(&core).await["first"], false, "设过了");
}

/// 本机令牌连上的直接改访问密码（施工 SU-1）：以前的设备全部作废、连着的断开；这个连接照旧是本机的，不另给设备令牌。
#[tokio::test]
async fn the_local_token_changes_the_password_and_drops_every_device() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let old = set_up(&core, "correct horse").await;
    let mut phone = Client::connect(Arc::clone(&core));
    assert!(
        hello_with(&mut phone, json!({"login": old}))
            .await
            .get("result")
            .is_some()
    );
    let mut local = local(&core).await;
    let reply = local
        .call("s", "account.setup", json!({"password": "battery staple"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(is_closed(&mut phone).await, "以前的设备断开");
    let mut stale = Client::connect(Arc::clone(&core));
    assert_eq!(
        why(&hello_with(&mut stale, json!({"login": old})).await),
        "bad_login"
    );
    let mut fresh = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut fresh, json!({"password": "battery staple"})).await;
    assert!(reply["result"]["login"]["token"].is_string(), "{reply}");
}

#[tokio::test]
async fn codes_expire_and_only_the_newest_sixteen_are_kept() {
    let home = Home::new();
    let core = home.core_code_ttl(Duration::from_millis(200));
    let stale = setup_code(&core).await["code"]
        .as_str()
        .expect("有码")
        .to_string();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut browser = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut browser, json!({"code": stale})).await;
    assert_eq!(why(&refused), "bad_code", "过期了：{refused}");
    let core = home.core(&Script::new([]));
    let mut client = local(&core).await;
    let mut codes = Vec::new();
    for n in 0..17 {
        let reply = client
            .call(&format!("c{n}"), "account.setup_code", json!({}))
            .await;
        codes.push(reply["result"]["code"].as_str().expect("有码").to_string());
    }
    let mut browser = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut browser, json!({"code": codes[0]})).await;
    assert_eq!(why(&refused), "bad_code", "多了丢最早的：{refused}");
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"code": codes[1]})).await;
    assert_eq!(reply["result"]["setup"], true, "{reply}");
}

#[tokio::test]
async fn passwords_log_in_and_too_many_failures_from_one_place_are_refused() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    // 还没设：密码一律不认；远程来的什么都没写的说清楚是没设。
    let mut early = remote(&core, "192.168.1.20").await;
    assert_eq!(
        why(&hello_with(&mut early, json!({"password": "correct horse"})).await),
        "password_not_set"
    );
    let mut bare = remote(&core, "192.168.1.20").await;
    assert_eq!(
        why(&hello_with(&mut bare, json!({})).await),
        "password_not_set"
    );
    set_up(&core, "correct horse").await;
    let mut bare = remote(&core, "192.168.1.20").await;
    assert_eq!(
        why(&hello_with(&mut bare, json!({})).await),
        "bad_token",
        "设了以后照常"
    );
    let mut right = remote(&core, "192.168.1.20").await;
    let reply = hello_with(&mut right, json!({"password": "correct horse"})).await;
    assert!(is_hex64(
        reply["result"]["login"]["token"].as_str().expect("{reply}")
    ));
    assert!(reply["result"]["login"]["expires"].is_string());
    // 同一个来源错 5 次，第 6 次连对的也先拒、不验。
    for n in 0..5 {
        let mut wrong = remote(&core, "192.168.1.20").await;
        let reply = hello_with(&mut wrong, json!({"password": "wrong one"})).await;
        assert_eq!(why(&reply), "bad_password", "第 {n} 次：{reply}");
    }
    let mut held = remote(&core, "192.168.1.20").await;
    let reply = hello_with(&mut held, json!({"password": "correct horse"})).await;
    assert_eq!(why(&reply), "login_throttled", "{reply}");
    assert!(
        reply["error"]["data"]["retry_after_ms"]
            .as_u64()
            .is_some_and(|ms| ms > 0)
    );
    // 别的来源、本机来的各算各的。
    let mut other = remote(&core, "192.168.1.21").await;
    assert!(
        hello_with(&mut other, json!({"password": "correct horse"}))
            .await
            .get("result")
            .is_some()
    );
    let mut here = Client::connect(Arc::clone(&core));
    assert!(
        hello_with(&mut here, json!({"password": "correct horse"}))
            .await
            .get("result")
            .is_some()
    );
}

#[tokio::test]
async fn login_tokens_are_known_until_they_expire_or_are_revoked() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let token = set_up(&core, "correct horse").await;
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"login": token})).await;
    assert!(reply.get("result").is_some(), "{reply}");
    assert!(reply["result"].get("login").is_none(), "用设备令牌的不另发");
    for bad in [&"0".repeat(64), "not hex"] {
        let mut stranger = Client::connect(Arc::clone(&core));
        let refused = hello_with(&mut stranger, json!({"login": bad})).await;
        assert_eq!(why(&refused), "bad_login", "{refused}");
        assert!(is_closed(&mut stranger).await);
    }
    // 过期了的不认：照文件写一个已经过期的。
    let path = home.root.path().join(logins::FILE);
    let read = logins::read(&path).expect("读得了");
    let mut written = read.logins.clone();
    let expired = "1".repeat(64);
    written.tokens.push(logins::Login {
        created: miyu_kernel::time::Timestamp::parse("2026-01-01T00:00:00.000Z").expect("合"),
        expires: miyu_kernel::time::Timestamp::parse("2026-01-31T00:00:00.000Z").expect("合"),
        hash: logins::digest(&expired),
    });
    logins::write(&path, &written, read.version.as_deref()).expect("写得进");
    let mut old = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut old, json!({"login": expired})).await;
    assert_eq!(why(&refused), "bad_login", "{refused}");
    // 重设密码：以前的登录、以前的密码都不认，新的认。
    let issued = setup_code(&core).await;
    assert_eq!(issued["first"], false, "设过了");
    let new_token = {
        let mut browser = Client::connect(Arc::clone(&core));
        hello_with(&mut browser, json!({"code": issued["code"]})).await;
        let reply = browser
            .call("s", "account.setup", json!({"password": "battery staple"}))
            .await;
        reply["result"]["login"]["token"]
            .as_str()
            .expect("{reply}")
            .to_string()
    };
    let mut stale = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut stale, json!({"login": token})).await;
    assert_eq!(
        why(&refused),
        "bad_login",
        "重设以后以前的登录作废：{refused}"
    );
    assert!(is_closed(&mut browser).await, "以前登着的连接断开");
    let mut stale = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut stale, json!({"password": "correct horse"})).await;
    assert_eq!(why(&refused), "bad_password", "{refused}");
    let mut fresh = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut fresh, json!({"login": new_token})).await;
    assert!(reply.get("result").is_some(), "{reply}");
    assert!(!access_of(&home).password.is_empty(), "还是那一份");
}

#[tokio::test]
async fn logout_revokes_one_or_all_and_closes_their_connections() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let first = set_up(&core, "correct horse").await;
    let mut a = Client::connect(Arc::clone(&core));
    hello_with(&mut a, json!({"login": first})).await;
    let mut b = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut b, json!({"password": "correct horse"})).await;
    let second = reply["result"]["login"]["token"]
        .as_str()
        .expect("有")
        .to_string();
    let mut c = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut c, json!({"password": "correct horse"})).await;
    assert!(reply.get("result").is_some());
    let mut terminal = local(&core).await;
    let reply = a.call("o", "account.logout", json!({})).await;
    assert_eq!(reply["result"]["revoked"], 1, "{reply}");
    assert!(is_closed(&mut a).await, "回完断开");
    let mut again = Client::connect(Arc::clone(&core));
    assert_eq!(
        why(&hello_with(&mut again, json!({"login": first})).await),
        "bad_login"
    );
    let listed = b.call("l", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "别的照旧：{listed}");
    // 本机的只能全部作废。
    let refused = terminal.call("o", "account.logout", json!({})).await;
    assert_eq!(why(&refused), "bad_params", "{refused}");
    let reply = b.call("o", "account.logout", json!({"all": true})).await;
    assert_eq!(reply["result"]["revoked"], 2, "{reply}");
    assert!(is_closed(&mut b).await);
    assert!(is_closed(&mut c).await, "用密码连着的也断开");
    let mut again = Client::connect(Arc::clone(&core));
    assert_eq!(
        why(&hello_with(&mut again, json!({"login": second})).await),
        "bad_login"
    );
    let listed = terminal.call("l", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "本机的不受影响：{listed}");
    let reply = terminal
        .call("o", "account.logout", json!({"all": true}))
        .await;
    assert_eq!(reply["result"]["revoked"], 0, "{reply}");
    assert!(logins_of(&home).tokens.is_empty());
    // 密码不动。
    let mut d = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut d, json!({"password": "correct horse"})).await;
    assert!(reply.get("result").is_some(), "{reply}");
}

#[tokio::test]
async fn only_one_kind_of_credential_is_accepted() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let code = setup_code(&core).await["code"]
        .as_str()
        .expect("有")
        .to_string();
    for credentials in [
        json!({"token": TOKEN, "code": code}),
        json!({"login": "a", "password": "c"}),
        json!({"ticket": "a", "password": "c"}),
    ] {
        let mut client = Client::connect(Arc::clone(&core));
        let refused = hello_with(&mut client, credentials.clone()).await;
        assert_eq!(why(&refused), "bad_params", "{credentials}: {refused}");
        assert!(is_closed(&mut client).await, "回完断开");
    }
    let mut client = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut client, json!({})).await;
    assert_eq!(why(&refused), "bad_token", "{refused}");
    // 码没被上面那次一起写的握手用掉。
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"code": code})).await;
    assert_eq!(reply["result"]["setup"], true, "{reply}");
}

#[tokio::test]
async fn broken_files_read_as_empty() {
    let home = Home::new();
    home.write("access.json", "not json");
    let core = home.core(&Script::new([]));
    let issued = setup_code(&core).await;
    assert_eq!(issued["first"], true, "坏了的当没设过：{issued}");
    let mut browser = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut browser, json!({"password": "whatever1"})).await;
    assert_eq!(why(&refused), "password_not_set", "{refused}");
    let token = set_up(&core, "correct horse").await;
    assert!(!access_of(&home).password.is_empty(), "坏的那份换成了好的");
    home.write("logins.json", "{");
    let mut stale = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut stale, json!({"login": token})).await;
    assert_eq!(why(&refused), "bad_login", "{refused}");
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"password": "correct horse"})).await;
    assert!(
        reply["result"]["login"]["token"].is_string(),
        "坏的那份换成了好的：{reply}"
    );
    assert_eq!(logins_of(&home).tokens.len(), 1);
}

#[tokio::test]
async fn tickets_are_for_people_and_redeem_once() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let code = setup_code(&core).await["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let mut half = Client::connect(Arc::clone(&core));
    hello_with(&mut half, json!({"code": code})).await;
    let refused = half.call("t", "login.ticket", json!({})).await;
    assert_eq!(
        why(&refused),
        "setup_first",
        "一次性码连上的只能设密码，要不了票"
    );

    let mut local = local(&core).await;
    let ticket = local.call("t", "login.ticket", json!({})).await;
    let value = ticket["result"]["ticket"]
        .as_str()
        .unwrap_or_else(|| panic!("{ticket}"))
        .to_string();
    assert!(is_hex64(&value), "{ticket}");
    assert!(ticket["result"]["expires"].is_string());
    let redeemed = local
        .call("r", "login.redeem", json!({"ticket": value}))
        .await;
    assert!(
        redeemed["result"].get("account").is_none(),
        "不带账号了：{redeemed}"
    );
    let token = redeemed["result"]["login"]["token"]
        .as_str()
        .expect("换出登录令牌")
        .to_string();
    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"login": token})).await;
    assert!(shaken.get("result").is_some(), "换出的认得：{shaken}");
    let again = local
        .call("r2", "login.redeem", json!({"ticket": value}))
        .await;
    assert_eq!(why(&again), "bad_ticket");
    let bad = local.call("r3", "login.redeem", json!({})).await;
    assert_eq!(why(&bad), "bad_params");
}

/// 文件在、密码是空的（施工 SU-6 上）：照样算没设访问密码。
#[tokio::test]
async fn an_access_file_without_a_password_is_not_a_password() {
    let home = Home::new();
    access::write(
        &home.root.path().join(access::FILE),
        &access::Access::default(),
        None,
    )
    .expect("写得进");
    let core = home.core(&Script::new([]));
    assert_eq!(setup_code(&core).await["first"], true, "还没设过");
    let mut bare = remote(&core, "192.168.1.30").await;
    assert_eq!(
        why(&hello_with(&mut bare, json!({})).await),
        "password_not_set"
    );
    let mut guess = remote(&core, "192.168.1.30").await;
    assert_eq!(
        why(&hello_with(&mut guess, json!({"password": "anything at all"})).await),
        "password_not_set"
    );
}

//! 身份的运行日志（施工 W-8，`docs/blueprint/web-module.md`「出错」的运行日志表）：设密码、造登录令牌、作废、握手被拒各记一行；
//! 扩展替本机页面要票记一行带包编号和 `via`，远程换不了记一行（施工 LP-1）；把能走的路都走一遍，整份运行日志里搜不到一次性码、
//! 密码、登录令牌、交接票，`TRACE` 也搜不到。
//!
//! 全局装一个写进内存的订阅者，一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::Script;

use support::extensions::{Program, approve, core, install_asking, quick, read, record, steps};
use support::*;

/// 批过 `login.page` 的扩展 `pages` 替本机页面要两张票：交回拉起它的核心和这两张票。
async fn page_tickets(home: &Home) -> (Arc<miyu_endpoint::Core>, Vec<String>) {
    let program = Program::new();
    let (path, keep) = record(home, "pages");
    let page = r#"ask:login.ticket:{"via":"page"}"#;
    let args = [keep.as_str(), "hello", page, page, "wait"];
    install_asking(
        home,
        "pages",
        &program.name(),
        "always",
        &steps(&args),
        &["login.page"],
    );
    approve(home, "pages", &["login.page"]);
    let extensions = core(home, quick());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 4 {
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    extensions.stop_extensions().await;
    let tickets = read(&path)
        .lines()
        .skip(2)
        .map(|line| {
            let reply: Value = serde_json::from_str(line).expect("是 JSON");
            reply["result"]["ticket"]
                .as_str()
                .unwrap_or_else(|| panic!("要得到票：{reply}"))
                .to_string()
        })
        .collect();
    (extensions, tickets)
}

async fn hello_with(client: &mut Client, credentials: Value) -> Value {
    let mut params = json!({"protocol": [1, 1], "head": {"kind": "web", "version": "0.0.0"}});
    if let (Some(params), Value::Object(more)) = (params.as_object_mut(), credentials) {
        params.extend(more);
    }
    client.call("h", "hello", params).await
}

#[tokio::test]
async fn the_log_says_what_happened_and_never_a_secret() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::TRACE,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut terminal = Client::connect(Arc::clone(&core));
    terminal.hello().await;
    let code = terminal.call("c", "account.setup_code", json!({})).await["result"]["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let password = "correct horse battery";
    let mut browser = Client::connect(Arc::clone(&core));
    hello_with(&mut browser, json!({"code": code})).await;
    let first = browser
        .call("s", "account.setup", json!({"password": password}))
        .await["result"]["login"]["token"]
        .as_str()
        .expect("有令牌")
        .to_string();
    let mut second = Client::connect(Arc::clone(&core));
    let token =
        hello_with(&mut second, json!({"password": password})).await["result"]["login"]["token"]
            .as_str()
            .expect("有令牌")
            .to_string();
    let mut third = Client::connect(Arc::clone(&core));
    hello_with(&mut third, json!({"login": first})).await;
    for credentials in [
        json!({"code": code}),
        json!({"login": "0".repeat(64)}),
        json!({"password": "wrong horse"}),
    ] {
        let mut stranger = Client::connect(Arc::clone(&core));
        hello_with(&mut stranger, credentials).await;
    }
    for _ in 0..5 {
        let mut stranger = Client::connect(Arc::clone(&core));
        hello_with(&mut stranger, json!({"password": "wrong horse"})).await;
    }
    terminal
        .call("o", "account.logout", json!({"all": true}))
        .await;
    // 扩展替本机页面要票（施工 LP-1）：本机换一张，远程拿另一张换不了。
    let pages = Home::new();
    let (core, tickets) = page_tickets(&pages).await;
    let mut here = Client::connect(Arc::clone(&core));
    hello_with(&mut here, json!({"ticket": tickets[0]})).await;
    let mut far = Client::connect(Arc::clone(&core));
    let notice = json!({"jsonrpc": "2.0", "method": "connection.remote", "params": {"from": "192.168.1.30"}});
    far.line(&notice.to_string()).await;
    hello_with(&mut far, json!({"ticket": tickets[1]})).await;
    let lines = memory.lines();
    let has = |what: &str| lines.iter().any(|line| line.contains(what));
    for wanted in [
        "password set first=true",
        "login issued via=setup",
        "login issued via=password",
        "via=code",
        "via=password",
        "via=login",
        "bad code",
        "bad login",
        "bad password",
        "login throttled",
        "logins revoked count=2",
        "login issued via=ticket",
        "ticket for this machine refused off it",
    ] {
        assert!(has(wanted), "{wanted}: {lines:#?}");
    }
    let issued: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("ticket issued") && line.contains("by=extension"))
        .collect();
    assert_eq!(issued.len(), 2, "一张一行：{lines:#?}");
    for line in issued {
        assert!(
            line.contains("purpose=page") && line.contains("package=pages"),
            "记包编号和 via：{line}"
        );
    }
    for secret in [
        code.as_str(),
        first.as_str(),
        token.as_str(),
        password,
        "wrong horse",
        tickets[0].as_str(),
        tickets[1].as_str(),
    ] {
        assert!(!has(secret), "运行日志里有 {secret}");
    }
}

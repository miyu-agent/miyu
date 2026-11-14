//! 用量框里的额度（施工 QU-1，`session.quota`）：真核心、真路由，会话用的是 opencode Go（档案写了查额度的地址，指着一台单独的
//! 假服务器）。说完一轮核心在后台查过一次，再问照它答、不再发；交三格，名字照连接的语言。档案没写的供应商、查不到的回 `{}`、
//! 不报错；没有这个会话的照找会话的说法拒。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_http::testkit::{Reply, Server};

use crate::support::providers::{data, profiles, routed, said};
use crate::support::*;

/// 2026-11-14 实测的回应。
const USAGE: &str = r#"{"usage":{"rolling":{"status":"ok","percent":4,"resetsAt":"2026-11-14T08:31:28.000Z"},"weekly":{"status":"ok","percent":12,"resetsAt":"2026-11-17T00:00:00.000Z"},"monthly":{"status":"ok","percent":30,"resetsAt":"2026-12-01T00:00:00.000Z"}}}"#;

fn json_reply(status: u16, body: &str) -> Reply {
    Reply::error(status, &[("Content-Type", "application/json")], body)
}

/// 界面中文，`opencode-go` 说话在 `talk`，key 照环境变量 `GO_KEY`，`models.chat` 指着它。
fn config(home: &Home, talk: &str) {
    home.write(
        "etc/config.toml",
        &format!(
            "[ui]\nlanguage = \"zh\"\n\n[providers.opencode-go]\ndriver = \"openai-chat\"\nbase_url = \"{talk}\"\nkey = {{ env = \"GO_KEY\" }}\n\n[models]\nchat = \"opencode-go/deepseek-v4.1-flash\"\n"
        ),
    );
}

/// 档案：`opencode-go` 的查额度地址是 `quota`，没有的不写。
fn go(quota: Option<String>) -> Value {
    let mut go = json!({"headers": {"x-opencode-session": "ses_{session_digest}"}});
    if let Some(url) = quota {
        go["quota"] = json!({"url": url, "format": "opencode-go"});
    }
    json!({"opencode-go": go})
}

/// 等 `done` 成立，最多 10 秒。
async fn settle(done: impl Fn() -> bool) {
    for _ in 0..1000 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("10 秒没等到");
}

#[tokio::test]
async fn a_go_session_gets_three_windows_checked_after_its_turn() {
    let talk = Server::start(vec![said("好的。"), said("好的。"), said("好的。")]).await;
    let usage = Server::start(vec![json_reply(200, USAGE)]).await;
    let home = Home::new();
    config(&home, &talk.base_url);
    let data = data(profiles(go(Some(format!("{}/usage", usage.base_url)))));
    let mut client = Client::connect(routed(&home, &[("GO_KEY", "sk-go")], data));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    settle(|| usage.received().len() == 1).await;
    assert_eq!(
        usage.received()[0].header("authorization"),
        Some("Bearer sk-go")
    );
    let reply = client
        .call("q1", "session.quota", json!({"session": session}))
        .await;
    assert_eq!(
        reply["result"],
        json!({"windows": [
            {"id": "rolling", "label": "5 小时", "percent": 4, "status": "ok",
             "resets_at": "2026-11-14T08:31:28.000Z"},
            {"id": "weekly", "label": "每周", "percent": 12, "status": "ok",
             "resets_at": "2026-11-17T00:00:00.000Z"},
            {"id": "monthly", "label": "每月", "percent": 30, "status": "ok",
             "resets_at": "2026-12-01T00:00:00.000Z"}
        ]}),
        "{reply}"
    );
    assert_eq!(usage.received().len(), 1, "60 秒内照后台查过的答");
}

#[tokio::test]
async fn no_quota_interface_or_a_failed_one_is_an_empty_answer() {
    for (quota, replies) in [
        (false, vec![json_reply(200, USAGE)]),
        (true, vec![json_reply(500, "oops")]),
        (true, vec![json_reply(404, "{}")]),
        (true, vec![json_reply(200, "not json")]),
    ] {
        let talk = Server::start(Vec::new()).await;
        let usage = Server::start(replies).await;
        let home = Home::new();
        config(&home, &talk.base_url);
        let url = quota.then(|| format!("{}/usage", usage.base_url));
        let mut client = Client::connect(routed(
            &home,
            &[("GO_KEY", "sk-go")],
            data(profiles(go(url))),
        ));
        client.hello().await;
        let session = client.create("c1", &home.work.to_string_lossy()).await;
        let reply = client
            .call("q1", "session.quota", json!({"session": session}))
            .await;
        assert_eq!(reply["result"], json!({}), "{reply}");
        assert_eq!(
            usage.received().len(),
            usize::from(quota),
            "档案没写的不发：{reply}"
        );
    }
}

#[tokio::test]
async fn an_unknown_session_or_bad_params_are_refused() {
    let home = Home::new();
    config(&home, "http://127.0.0.1:9/v1");
    let mut client = Client::connect(routed(&home, &[], data(profiles(go(None)))));
    client.hello().await;
    for (id, params, expected) in [
        (
            "r1",
            json!({"session": "0192f3a0-1111-7abc-8def-001122334455"}),
            "session_not_found",
        ),
        ("r2", json!({"session": "nope"}), "bad_params"),
        ("r3", json!({}), "bad_params"),
    ] {
        let reply = client.call(id, "session.quota", params).await;
        assert_eq!(reason(&reply), Some(expected), "{reply}");
    }
}

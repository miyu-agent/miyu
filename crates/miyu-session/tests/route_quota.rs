//! 查供应商的额度（施工 QU-1，`docs/blueprint/models.md`「怎么走」第十七条）：档案写了 `quota` 的一家 GET 那个地址，key 在
//! `Authorization` 里，另配的头照固定的种子换；读成三个窗口；60 秒内再问不再发；档案没写的不发；404、500、乱码当没有、不报错；
//! 配置能改地址；会话一轮结束在后台查一次。
//!
//! 额度的地址指着单独的一台假服务器，和说话的那台分开：收到几个请求只算额度的。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::Home;
use crate::support::routing::{configs, hellos, resolved, routes_with, turn};
use miyu_config::secret::{Reference, Secret};
use miyu_http::testkit::{Reply, Server};
use miyu_http::{Proxy, fetcher};
use miyu_models::headers::digest;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_session::{ModelData, Observed, quota};

/// 2026-11-14 实测的回应。
const USAGE: &str = r#"{"usage":{"rolling":{"status":"ok","percent":4,"resetsAt":"2026-11-14T08:31:28.000Z"},"weekly":{"status":"ok","percent":12,"resetsAt":"2026-11-17T00:00:00.000Z"},"monthly":{"status":"ok","percent":30,"resetsAt":"2026-12-01T00:00:00.000Z"}}}"#;

fn json_reply(status: u16, body: &str) -> Reply {
    Reply::error(status, &[("Content-Type", "application/json")], body)
}

/// 档案：`opencode-go` 照出厂的头，查额度的地址是 `quota`（没有的不写）。能拉东西（不走代理）、没有目录。
fn data(quota: Option<&str>) -> Arc<ModelData> {
    let mut go = json!({"headers": {"x-opencode-session": "ses_{session_digest}"}});
    if let Some(url) = quota {
        go["quota"] = json!({"url": url, "format": "opencode-go"});
    }
    let profiles = Profiles::parse(&json!({"providers": {"opencode-go": go}})).expect("档案写法对");
    let data = ModelData::new(profiles, Vendors::default(), None)
        .with_fetcher(fetcher(Proxy::Off).expect("造得出客户端"));
    data.loaded(None, Observed::default());
    Arc::new(data)
}

/// 说话的地址是 `base_url`，key 是 `{ env = "GO_KEY" }`，`models.chat` 指着它；另写 `more`。
fn source(base_url: &str, more: &str) -> String {
    format!(
        "[providers.opencode-go]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkey = {{ env = \"GO_KEY\" }}\n{more}\n[models]\nchat = \"opencode-go/m\"\n"
    )
}

fn key() -> Vec<(Reference, &'static str)> {
    vec![(Reference::Env("GO_KEY".to_string()), "sk-go")]
}

fn secret(reference: &Reference) -> Option<Secret> {
    (*reference == Reference::Env("GO_KEY".to_string()))
        .then(|| Secret::new("sk-go").expect("合写法"))
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

/// 问一次 `opencode-go` 的额度，写成 `[[编号, 百分比]…]`；没有的是 `None`。
async fn ask(data: &ModelData, more: &str) -> Option<Vec<(String, Value)>> {
    let values = resolved(&source("http://127.0.0.1:9/v1", more)).values();
    let windows = quota(data, &values, &secret, "opencode-go").await?;
    Some(
        windows
            .iter()
            .map(|window| (window.id.as_str().to_string(), json!(window.percent)))
            .collect(),
    )
}

#[tokio::test]
async fn the_go_quota_reads_three_windows_with_the_key_and_is_reused_for_a_minute() {
    let server = Server::start(vec![json_reply(200, USAGE), json_reply(200, USAGE)]).await;
    let data = data(Some(&format!("{}/usage", server.base_url)));
    let windows = ask(&data, "").await.expect("查得到");
    let ids: Vec<&str> = windows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["rolling", "weekly", "monthly"]);
    assert_eq!(windows[0].1, json!(4));
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, "GET");
    assert_eq!(received[0].path, "/v1/usage");
    assert_eq!(received[0].header("authorization"), Some("Bearer sk-go"));
    let wanted = format!("ses_{}", digest("quota"));
    assert_eq!(
        received[0].header("x-opencode-session"),
        Some(wanted.as_str())
    );
    let agent = received[0].header("user-agent").unwrap_or_default();
    assert!(agent.starts_with("miyu/"), "不是默认库的：{agent}");
    assert_eq!(ask(&data, "").await, Some(windows), "60 秒内照上一次的");
    assert_eq!(server.received().len(), 1, "60 秒内不再发");
}

#[tokio::test]
async fn a_provider_without_a_quota_is_not_asked() {
    let server = Server::start(vec![json_reply(200, USAGE)]).await;
    let data = data(None);
    let more = format!(
        "[providers.opencode-go.quota]\nurl = \"{}/usage\"\n",
        server.base_url
    );
    assert_eq!(ask(&data, &more).await, None);
    assert!(
        server.received().is_empty(),
        "档案没写格式的，配置写了地址也不查"
    );
}

#[tokio::test]
async fn the_config_moves_the_address() {
    let server = Server::start(vec![json_reply(200, USAGE)]).await;
    let data = data(Some("http://127.0.0.1:9/never"));
    let more = format!(
        "[providers.opencode-go.quota]\nurl = \"{}/moved\"\n",
        server.base_url
    );
    assert!(ask(&data, &more).await.is_some());
    assert_eq!(server.received()[0].path, "/v1/moved");
}

#[tokio::test]
async fn a_failed_or_garbled_reply_is_no_quota_and_not_asked_again_at_once() {
    for reply in [
        json_reply(404, r#"{"error":"not found"}"#),
        json_reply(500, "oops"),
        json_reply(200, "<html>not json</html>"),
        json_reply(200, r#"{"usage":{"rolling":{"percent":"4"}}}"#),
    ] {
        let status = reply.status;
        let server = Server::start(vec![reply, json_reply(200, USAGE)]).await;
        let data = data(Some(&format!("{}/usage", server.base_url)));
        assert_eq!(ask(&data, "").await, None, "{status}");
        assert_eq!(ask(&data, "").await, None, "{status}");
        assert_eq!(server.received().len(), 1, "{status}：查不到的也算一次");
    }
}

#[tokio::test]
async fn a_turn_ending_asks_once_in_the_background() {
    let talk = Server::start(hellos(4)).await;
    let usage = Server::start(vec![json_reply(200, USAGE)]).await;
    let mut home = Home::new();
    home.configs = configs(&source(&talk.base_url, ""), &key());
    let data = data(Some(&format!("{}/usage", usage.base_url)));
    let handle = home
        .create(&routes_with(Arc::clone(&data), Duration::from_secs(60)))
        .await;
    assert!(usage.received().is_empty(), "还没说话不查");
    turn(&handle, "cmd-1").await;
    settle(|| usage.received().len() == 1).await;
    assert_eq!(
        usage.received()[0].header("authorization"),
        Some("Bearer sk-go")
    );
    // 后台查过的留着：这时再问照它答，不再发。
    assert!(ask(&data, "").await.is_some());
    assert_eq!(usage.received().len(), 1);
}

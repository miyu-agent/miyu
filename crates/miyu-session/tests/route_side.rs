//! 旁路请求照声明的档位用池（施工 Q-2，设计 `43-旁路请求和档位池.md` 第三节第 2、3 条）：起标题登记的是 `lite`。两台假服务器
//! 各是一家：会话的模型 `a/m` 在一台，`lite` 池的成员在另一台。池有成员的，起标题发给池里的、不动会话钉着的；池是空的，用会话
//! 的模型；池的成员都出错了，退回会话的模型，`model.called` 记 `fell_back`。主请求照旧发给会话的模型。

use std::time::Duration;

use crate::support::routing::{called, configs, hellos, routes, turn};
use crate::support::{Home, Lines, Opening, until_logged};
use miyu_http::testkit::{Reply, Server};
use miyu_kernel::event::{Body, Purpose};
use miyu_tool::Catalog;

/// 会话的模型 `a/m` 在 `session` 上，`b` 在 `lite` 上；`lite` 池的成员照 `members` 写。
fn config(session: &Server, lite: &Server, members: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n\n[pools.lite]\nmodels = [{members}]\n",
        session.base_url, lite.base_url
    )
}

/// 说一句，等起标题那一条 `model.called` 落了盘；交回起标题发给了谁（`<供应商>/<模型>`）、退过没有，和主请求发给了谁。
async fn title_and_main(home: &Home) -> ((String, bool), Vec<String>) {
    title_and_main_on(home, None).await
}

/// 同 [`title_and_main`]，会话钉着的模型是 `model`（没有的照 `models.chat`）。
async fn title_and_main_on(home: &Home, model: Option<&str>) -> ((String, bool), Vec<String>) {
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let lines = Lines {
        model: model.map(str::to_string),
        ..Lines::default()
    };
    let handle = home
        .create_full(&routes, &Catalog::default(), Opening::default(), lines)
        .await;
    turn(&handle, "cmd-1").await;
    until_logged(home, handle.id(), |log| {
        log.iter().any(|event| {
            matches!(&event.body, Body::ModelCalled(call) if call.purpose == Some(Purpose::Title) && call.endpoint.is_some())
        })
    })
    .await;
    let calls = called(home, &handle);
    let who = |call: &miyu_kernel::event::ModelCalled| {
        format!(
            "{}/{}",
            call.endpoint
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            call.model
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default()
        )
    };
    let title = calls
        .iter()
        .find(|call| call.purpose == Some(Purpose::Title) && call.endpoint.is_some())
        .map(|call| (who(call), call.fell_back))
        .expect("起过标题");
    let main = calls
        .iter()
        .filter(|call| call.purpose.is_none())
        .map(who)
        .collect();
    (title, main)
}

#[tokio::test]
async fn the_title_goes_to_the_lite_pool_and_main_requests_stay_put() {
    let (session, lite) = (
        Server::start(hellos(4)).await,
        Server::start(hellos(4)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&session, &lite, r#""b/y""#), &[]);
    let (title, main) = title_and_main(&home).await;
    assert_eq!(title, ("b/y".to_string(), false), "起标题照 lite 池");
    assert_eq!(main, ["a/m"], "主请求照会话的模型");
}

#[tokio::test]
async fn an_empty_lite_pool_leaves_the_title_on_the_session_model() {
    let (session, lite) = (
        Server::start(hellos(4)).await,
        Server::start(Vec::new()).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&session, &lite, ""), &[]);
    let (title, _) = title_and_main(&home).await;
    assert_eq!(
        title,
        ("a/m".to_string(), false),
        "池是空的用会话的模型，不算退"
    );
    assert!(lite.received().is_empty(), "空池一次都没发");
}

#[tokio::test]
async fn a_failing_lite_pool_falls_back_to_the_session_model() {
    let session = Server::start(hellos(4)).await;
    let lite = Server::start(vec![Reply::error(500, &[], "boom"); 4]).await;
    let mut home = Home::new();
    home.configs = configs(&config(&session, &lite, r#""b/y""#), &[]);
    let (title, main) = title_and_main(&home).await;
    assert_eq!(
        title,
        ("a/m".to_string(), true),
        "池的成员出错了，退回会话的模型"
    );
    assert_eq!(main, ["a/m"]);
    assert!(!lite.received().is_empty(), "先试过池里的");
}

/// 会话的模型本身就是 `lite` 池（主会话 2026-10-26 要守着）：池里那一个出错了，起标题不再往下退（免得来回退），照池的规矩报；
/// 没碰 `models.chat` 那一台，`fell_back` 不写。
#[tokio::test]
async fn a_session_riding_the_lite_pool_does_not_fall_back_any_further() {
    let session = Server::start(hellos(4)).await;
    let mut replies = hellos(1);
    replies.extend(vec![Reply::error(500, &[], "boom"); 6]);
    let lite = Server::start(replies).await;
    let mut home = Home::new();
    home.configs = configs(&config(&session, &lite, r#""b/y""#), &[]);
    let (title, main) = title_and_main_on(&home, Some("@lite")).await;
    assert_eq!(main, ["b/y"], "主请求照会话钉着的 lite 池");
    assert_eq!(
        title,
        ("b/y".to_string(), false),
        "起标题照 lite 池，出错了也不退"
    );
    assert!(
        session.received().is_empty(),
        "models.chat 那一台一次都没发"
    );
}

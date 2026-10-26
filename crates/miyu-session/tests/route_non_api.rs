//! 2xx 但不是 API 应答（施工 Q-4 中，设计 43 第三节第 5 条第 3 款）：中转站、网关常回一页 HTML（登录页、维护页）、空的响应体，
//! 状态却是 200。驱动拿不到一个可解析的应答，报可重试，池里照样记冷却、换下一个成员。流里已经给过内容再断的照原来的断流办，
//! 不在这里（主会话要求：不换成员重来，免得把已经给人看的那半截再生成一遍）。

use std::time::Duration;

use crate::support::routing::{called, configs, hellos, routes};
use crate::support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};
use miyu_http::testkit::{Reply, Server};
use miyu_session::{Handle, Models};
use miyu_tool::Catalog;

/// 两家，池 `p` 是 `a/x`、`b/y`，钉住；`models.chat` 是 `a/m`。`a` 用驱动 `driver`，`b` 照样本回 openai-chat 的流。
fn pooled(first: &Server, second: &Server, driver: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"{driver}\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n\n[pools.p]\nmodels = [\"a/x\", \"b/y\"]\nstrategy = \"pin\"\n",
        first.base_url, second.base_url
    )
}

async fn create(home: &Home, models: &dyn Models) -> Handle {
    let lines = Lines {
        model: Some("@p".to_string()),
        ..Lines::default()
    };
    home.create_full(models, &Catalog::default(), Opening::default(), lines)
        .await
}

/// 主请求发给了谁、怎么收场（`<供应商>/<模型>:<分类或 ok>`），照先后。
fn sent(home: &Home, handle: &Handle) -> Vec<String> {
    called(home, handle)
        .iter()
        .filter(|call| call.purpose.is_none())
        .map(|call| {
            format!(
                "{}/{}:{}",
                call.endpoint.as_ref().map_or("?", |id| id.as_str()),
                call.model.as_ref().map_or("?", |name| name.as_str()),
                call.error
                    .as_ref()
                    .map_or("ok", |error| error.class.as_str())
            )
        })
        .collect()
}

/// 第一家回 `reply`，三种驱动各看一遍这一轮发给了谁：都该是第一家可重试、换到第二家成了。
async fn first_says(reply: Reply) {
    for driver in ["openai-chat", "anthropic", "openai-responses"] {
        let first = Server::start(vec![reply.clone()]).await;
        let second = Server::start(hellos(4)).await;
        let mut home = Home::new();
        home.configs = configs(&pooled(&first, &second, driver), &[]);
        let routes = routes(serde_json::json!({}), Duration::from_secs(60));
        let handle = create(&home, &routes).await;
        let mut pushes = watch(&handle).await;
        ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
        assert_eq!(
            sent(&home, &handle),
            ["a/x:retryable", "b/y:ok"],
            "{driver}"
        );
    }
}

#[tokio::test]
async fn a_200_html_page_is_retryable_and_fails_over() {
    let page = Reply::error(
        200,
        &[("Content-Type", "text/html; charset=utf-8")],
        "<!doctype html><html><body>Please sign in</body></html>",
    );
    first_says(page).await;
}

#[tokio::test]
async fn a_200_empty_body_is_retryable_and_fails_over() {
    let empty = Reply::error(200, &[("Content-Type", "text/event-stream")], "");
    first_says(empty).await;
}

#[tokio::test]
async fn a_200_json_that_is_not_an_answer_is_retryable_and_fails_over() {
    let json = Reply::error(
        200,
        &[("Content-Type", "application/json")],
        r#"{"status":"ok"}"#,
    );
    first_says(json).await;
}

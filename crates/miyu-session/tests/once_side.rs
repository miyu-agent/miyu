//! 一次性入口照登记的档位发旁路请求（施工 Q-2，设计 `43-旁路请求和档位池.md` 第三节第 3 条，`OneShot::call_side`）：整理记忆
//! 登记的是 `normal`。池有成员的发给池里的；池是空的照后备的引用（请求所属会话这一刻的，没有的照 `models.chat`）；池里换遍了
//! 还没答成的退到后备再发一次；后备就是这个池的不再退。

use std::time::Duration;

use serde_json::json;

use crate::support::calling::{asking, blobs, entry, frozen};
use crate::support::routing::{hellos, routes};
use miyu_http::testkit::{Reply, Server};
use miyu_models::side::ORGANIZE;
use miyu_session::Unanswered;

/// `a/m` 在 `session` 上（`models.chat` 也是它），`b` 在 `pool` 上；`normal` 池的成员照 `members` 写。
fn config(session: &Server, pool: &Server, members: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n\n[pools.normal]\nmodels = [{members}]\n",
        session.base_url, pool.base_url
    )
}

/// 照整理记忆的登记发一次，后备是 `fallback`；交回真发给的 `<供应商>/<模型>`。
async fn organize(config: &str, fallback: Option<&str>) -> Result<String, Unanswered> {
    let config = frozen(config, &[]);
    let routes = routes(json!({}), Duration::from_secs(60));
    let (_scratch, blobs) = blobs();
    entry(&routes)
        .call_side(
            &config,
            &blobs,
            asking(None, "memory", "hi"),
            &ORGANIZE,
            fallback.map(str::to_string),
        )
        .await
        .map(|answer| format!("{}/{}", answer.provider, answer.model))
}

#[tokio::test]
async fn organizing_goes_to_the_normal_pool() {
    let (session, pool) = (
        Server::start(hellos(2)).await,
        Server::start(hellos(2)).await,
    );
    let answered = organize(&config(&session, &pool, r#""b/y""#), Some("a/m")).await;
    assert_eq!(answered, Ok("b/y".to_string()));
    assert!(session.received().is_empty(), "会话的模型一次都没发");
}

#[tokio::test]
async fn an_empty_normal_pool_uses_the_fallback_or_the_chat_model() {
    let (session, pool) = (
        Server::start(hellos(2)).await,
        Server::start(Vec::new()).await,
    );
    let text = config(&session, &pool, "");
    assert_eq!(
        organize(&text, Some("a/m")).await,
        Ok("a/m".to_string()),
        "后备照会话的"
    );
    assert_eq!(
        organize(&text, None).await,
        Ok("a/m".to_string()),
        "没有会话的照 models.chat"
    );
    assert!(pool.received().is_empty());
}

#[tokio::test]
async fn a_failing_normal_pool_falls_back_once() {
    let session = Server::start(hellos(2)).await;
    let pool = Server::start(vec![Reply::error(500, &[], "boom"); 4]).await;
    let answered = organize(&config(&session, &pool, r#""b/y""#), Some("a/m")).await;
    assert_eq!(
        answered,
        Ok("a/m".to_string()),
        "池里出错了，退到会话的模型"
    );
    assert!(!pool.received().is_empty(), "先试过池里的");
}

#[tokio::test]
async fn a_fallback_on_the_same_pool_does_not_fall_back_again() {
    let session = Server::start(hellos(2)).await;
    let pool = Server::start(vec![Reply::error(500, &[], "boom"); 4]).await;
    let answered = organize(&config(&session, &pool, r#""b/y""#), Some("@normal")).await;
    assert!(
        matches!(answered, Err(Unanswered::Failed(_))),
        "后备就是这个池，照池的规矩报：{answered:?}"
    );
    assert!(session.received().is_empty(), "没往别处退");
}

/// 池里的成员这时备不好（key 取不到，「没有能用的模型」）：同样退到后备。
#[tokio::test]
async fn a_pool_member_without_a_key_falls_back_too() {
    let (session, pool) = (
        Server::start(hellos(2)).await,
        Server::start(hellos(2)).await,
    );
    let text = config(&session, &pool, r#""b/y""#).replace(
        "[providers.b]\n",
        "[providers.b]\nkey = { env = \"MIYU_Q2_UNSET_KEY\" }\n",
    );
    assert_eq!(organize(&text, Some("a/m")).await, Ok("a/m".to_string()));
    assert!(pool.received().is_empty(), "取不到 key 的没发");
}

/// 池里额度用完（402，施工 Q-4 中）也算会换人的错：照样退到后备。
#[tokio::test]
async fn an_out_of_quota_normal_pool_falls_back_too() {
    let session = Server::start(hellos(1)).await;
    let pool = Server::start(vec![
        Reply::error(
            402,
            &[],
            r#"{"error":{"message":"Insufficient Balance"}}"#
        );
        4
    ])
    .await;
    let answered = organize(&config(&session, &pool, r#""b/y""#), Some("a/m")).await;
    assert_eq!(answered, Ok("a/m".to_string()), "额度用完，退到会话的模型");
    assert!(!pool.received().is_empty(), "先试过池里的");
}

//! 池的路由补齐（施工 Q-4 上，设计 `43-旁路请求和档位池.md` 第三节第 5 条第 1、2 款）：照先后的池每次从第一个成员排起，出错换
//! 到后面的，前面的恢复了回到前面；会话的主请求带着内核算的用量（同压缩判线），窗口放不下的成员不挑、出错也不换过去。两台
//! 假服务器各是一家：`a` 的 `x` 窗口 64000，`b` 的 `y` 窗口 32000。

use std::sync::Arc;
use std::time::Duration;

use crate::support::routing::{called, configs, hellos, routes};
use crate::support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};
use miyu_http::testkit::{Reply, Server};
use miyu_models::cooldown::{Rule, Rules};
use miyu_session::{Handle, Models, Pushed};
use miyu_tool::Catalog;

/// 两家都不带 key，池 `p` 是 `a/x`、`b/y`，分法 `strategy`；`models.chat` 是 `a/m`。
fn pooled(first: &Server, second: &Server, strategy: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.x]\nwindow = 64000\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.y]\nwindow = 32000\n\n[models]\nchat = \"a/m\"\n\n[pools.p]\nmodels = [\"a/x\", \"b/y\"]\nstrategy = \"{strategy}\"\n",
        first.base_url, second.base_url
    )
}

/// 一次 503：可重试，会冷却、会换人。
fn down() -> Reply {
    Reply::error(503, &[], r#"{"error":{"message":"Service unavailable"}}"#)
}

/// 三类冷却都是 `millis` 毫秒：出错了这一次换人（内核当场再来时它还在冷却），过了这么久恢复。
fn cooling_for(millis: u64) -> Rules {
    let rule = Rule {
        base: Duration::from_millis(millis),
        max: Duration::from_millis(millis),
    };
    Rules {
        rate_limited: rule,
        retryable: rule,
        auth: rule,
    }
}

async fn create(home: &Home, models: &dyn Models) -> Handle {
    let lines = Lines {
        model: Some("@p".to_string()),
        ..Lines::default()
    };
    home.create_full(models, &Catalog::default(), Opening::default(), lines)
        .await
}

/// 说 `text`、等这一轮说完。
async fn turn(handle: &Handle, command: &str, text: &str) -> Vec<Arc<Pushed>> {
    let mut pushes = watch(handle).await;
    ask(handle, command, say(text)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await
}

/// 主请求发给了谁（`<供应商>/<模型>`），出错的也算，照先后。
fn sent(home: &Home, handle: &Handle) -> Vec<String> {
    called(home, handle)
        .iter()
        .filter(|call| call.purpose.is_none())
        .filter_map(|call| {
            Some(format!(
                "{}/{}",
                call.endpoint.as_ref()?,
                call.model.as_ref()?
            ))
        })
        .collect()
}

#[tokio::test]
async fn an_ordered_pool_starts_from_the_first_member_every_time() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&pooled(&first, &second, "ordered"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let one = create(&home, &routes).await;
    let other = create(&home, &routes).await;
    for n in 1..=2 {
        turn(&one, &format!("cmd-{n}"), "hi").await;
        turn(&other, &format!("cmd-{n}"), "hi").await;
    }
    assert_eq!(sent(&home, &one), ["a/x", "a/x"]);
    assert_eq!(
        sent(&home, &other),
        ["a/x", "a/x"],
        "新会话也从第一个起，不照指针分开"
    );
    assert_eq!(one.limits().window, Some(64_000), "限额照主力");
}

#[tokio::test]
async fn an_ordered_pool_falls_to_the_next_and_comes_back_when_the_first_recovers() {
    let first = Server::start([vec![down()], hellos(4)].concat()).await;
    let second = Server::start(hellos(4)).await;
    let mut home = Home::new();
    home.configs = configs(&pooled(&first, &second, "ordered"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    routes.data.set_cooldown_rules(cooling_for(300));
    let handle = create(&home, &routes).await;
    turn(&handle, "cmd-1", "hi").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    turn(&handle, "cmd-2", "hi").await;
    assert_eq!(
        sent(&home, &handle),
        ["a/x", "b/y", "a/x"],
        "第一个出错换到第二个，下一次回到第一个"
    );
}

#[tokio::test]
async fn a_member_whose_window_cannot_hold_the_request_is_not_switched_to() {
    let first = Server::start([vec![down()], hellos(4)].concat()).await;
    let second = Server::start(hellos(4)).await;
    let mut home = Home::new();
    home.configs = configs(&pooled(&first, &second, "pin"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    routes.data.set_cooldown_rules(cooling_for(300));
    let handle = create(&home, &routes).await;
    // 约四万个 token：放得进 `a/x` 的 64000，放不进 `b/y` 的 32000。
    let long = "word ".repeat(32_000);
    turn(&handle, "cmd-1", &long).await;
    let sent = sent(&home, &handle);
    assert!(
        !sent.contains(&"b/y".to_string()),
        "放不下的不换过去：{sent:?}"
    );
    assert_eq!(sent.first().map(String::as_str), Some("a/x"), "{sent:?}");
}

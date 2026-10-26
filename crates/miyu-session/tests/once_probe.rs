//! 全冷却只探一个（施工 Q-4 下，设计 `43-旁路请求和档位池.md` 第三节第 5 条第 4 款，照 magpie 的半开探测）：池里没有一个健康的
//! 成员（每个不是在冷却、就是冷却刚过还没成功过）时，这一次只试最早恢复的那一个，失败了不再往下换，不一口气把刚恢复的全打
//! 一遍；它成了，失败次数清零，池就回来了。有健康的成员时照旧，出错换下一个。走一次性入口：它在一次调用里自己换人，换没换
//! 看得出来。

use std::time::Duration;

use serde_json::json;

use crate::support::calling::{asking, blobs, entry, frozen};
use crate::support::routing::{hellos, routes};
use miyu_http::testkit::{Reply, Server};
use miyu_models::cooldown::{Rule, Rules};

/// 两家：`a`、`b` 各在一台服务器上，池 `p` 是 `a/x`、`b/y`，照先后。
fn pooled(a: &Server, b: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[pools.p]\nmodels = [\"a/x\", \"b/y\"]\nstrategy = \"ordered\"\n",
        a.base_url, b.base_url
    )
}

/// 一次 503。
fn down() -> Reply {
    Reply::error(503, &[], r#"{"error":{"message":"Service unavailable"}}"#)
}

/// 四类冷却都是 300 毫秒。
fn short() -> Rules {
    let rule = Rule {
        base: Duration::from_millis(300),
        max: Duration::from_millis(300),
    };
    Rules {
        rate_limited: rule,
        retryable: rule,
        auth: rule,
        quota: rule,
    }
}

#[tokio::test]
async fn when_no_member_is_healthy_only_the_earliest_recovered_one_is_tried() {
    let a = Server::start(vec![down(), down(), down()]).await;
    let b = Server::start([vec![down()], hellos(2)].concat()).await;
    let config = frozen(&pooled(&a, &b), &[]);
    let routes = routes(json!({}), Duration::from_secs(60));
    routes.data.set_cooldown_rules(short());
    let entry = entry(&routes);
    let (_scratch, blobs) = blobs();
    let ask = || asking(Some("@p"), "platform", "hi");
    assert!(
        entry.call(&config, &blobs, ask()).await.is_err(),
        "两个都出错"
    );
    assert_eq!((a.received().len(), b.received().len()), (1, 1));
    tokio::time::sleep(Duration::from_millis(400)).await;
    // 两个都冷却过了、都还没成功过：只探最早恢复的 `a`，它又出错，这一次就到此为止，不换到 `b`。
    assert!(
        entry.call(&config, &blobs, ask()).await.is_err(),
        "探的那一个出错了"
    );
    assert_eq!(
        (a.received().len(), b.received().len()),
        (2, 1),
        "只探了一个"
    );
    // 下一次：`a` 又在冷却，`b` 是剩下唯一恢复了的，探它；成了。
    let answer = entry.call(&config, &blobs, ask()).await.expect("探 b 成了");
    assert_eq!(answer.provider, "b");
}

#[tokio::test]
async fn with_a_healthy_member_an_error_still_moves_on() {
    let a = Server::start(vec![down(), down()]).await;
    let b = Server::start(hellos(2)).await;
    let config = frozen(&pooled(&a, &b), &[]);
    let routes = routes(json!({}), Duration::from_secs(60));
    routes.data.set_cooldown_rules(short());
    let entry = entry(&routes);
    let (_scratch, blobs) = blobs();
    let ask = || asking(Some("@p"), "platform", "hi");
    let answer = entry
        .call(&config, &blobs, ask())
        .await
        .expect("换到 b 成了");
    assert_eq!(answer.provider, "b");
    tokio::time::sleep(Duration::from_millis(400)).await;
    // `a` 冷却过了还没成功过，`b` 是健康的：照旧从 `a` 试起，出错换到 `b`。
    let answer = entry
        .call(&config, &blobs, ask())
        .await
        .expect("换到 b 成了");
    assert_eq!(answer.provider, "b");
    assert_eq!((a.received().len(), b.received().len()), (2, 2));
}

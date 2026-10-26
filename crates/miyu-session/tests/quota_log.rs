//! 额度用完（施工 Q-4 中，设计 43 第三节第 5 条第 3 款）：池里一个成员回 402，分类 `quota`，冷却到供应商说的重置时刻、换到池里
//! 别的成员；额度用完是账号级的事，记一行 `WARN provider out of quota`，写明哪一家、哪个模型、冷却到几点，人一看就知道「不是
//! 网断了是没钱了」（主会话要求）。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::time::Duration;

use miyu_http::testkit::{Reply, Server};
use miyu_log::{LevelFilter, Memory};
use support::routing::{called, configs, hellos, routes};
use support::{Home, ask, say, until_turn_ends, watch};

#[tokio::test]
async fn out_of_quota_is_warned_with_the_provider_and_until_when_and_fails_over() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let mut replies = vec![Reply::error(
        402,
        &[("retry-after", "7200")],
        r#"{"error":{"message":"Insufficient Balance","code":"insufficient_balance"}}"#,
    )];
    replies.extend(hellos(4));
    let server = Server::start(replies).await;
    let source = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{url}\"\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{url}\"\n\n\
         [pools.p]\nmodels = [\"a/m\", \"b/m\"]\nstrategy = \"pin\"\n\n[models]\nchat = \"@p\"\n",
        url = server.base_url
    );
    let mut home = Home::new();
    home.configs = configs(&source, &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = home.create(&routes).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;

    let sent: Vec<String> = called(&home, &handle)
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
        .collect();
    assert_eq!(sent, ["a/m:quota", "b/m:ok"], "额度用完换到别的成员");
    let lines = memory.lines();
    let session = handle.id().as_str();
    let warned = lines.iter().find(|line| {
        line.contains(&format!(
            " WARN  session  {session} provider out of quota provider=a model=m until="
        ))
    });
    let Some(warned) = warned else {
        panic!("该有一行 WARN provider out of quota：{lines:#?}");
    };
    assert!(
        lines.iter().any(|line| line
            .contains("endpoint cooling provider=a model=m class=quota for_ms=7200000 failures=1")),
        "冷却照供应商说的两小时：{lines:#?}"
    );
    assert!(warned.contains("T"), "到几点写成时刻：{warned}");
}

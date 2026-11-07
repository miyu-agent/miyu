//! `driver.check`（施工 MD-3，设计 66 第七节第 2 条，core1 11-07 定）：真核心、真扩展进程，扩展照 `driver:<…>+<怎么编>` 答
//! `driver.encode`。
//!
//! - 编得稳的：六份（五份样本加一对）都过，扩展真重启了（进程号变了）；
//! - 故意放时间戳的：当场红，说同一个请求两次编得不一样；只在重启以后才变的、末尾加一条消息稳定区之前就变的，各红各的；
//! - 没实现 `driver.encode` 的：`contract` 算红，别的算跳过；
//! - 有会话正经它调模型的：回 `driver_busy`，不重启。

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::providers::{data, profiles, routed_serving_with};
use crate::support::*;

/// 装包 `ag`：扩展照 `steps` 跑，声明驱动 `ag`，缓存类型 `cache`；批了 `model.driver`。
fn install(home: &Home, program: &str, steps: &[String], cache: &str) {
    let args: Vec<String> = steps.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        "opt/ag/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Ag\" }}\n\n[command]\nname = \"ag\"\nprogram = \"{program}\"\nabout = {{ en = \"A\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"ag\"\ncache = \"{cache}\"\n",
            args.join(", ")
        ),
    );
    approve(home, "ag", &["model.driver"]);
}

/// 起核心、连上、等扩展握了手；`g` 这一家走扩展驱动 `ag/ag`。
async fn connected(home: &Home) -> (Client, std::sync::Arc<miyu_endpoint::Core>) {
    home.write(
        "etc/config.toml",
        "[providers.g]\ndriver = \"ag/ag\"\n\n[models]\nchat = \"g/m\"\n",
    );
    let core = routed_serving_with(home, &[], data(profiles(json!({}))));
    core.start_drivers();
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    until_state(&mut client, "ag", |one| one["state"] == "running").await;
    (client, core)
}

/// 照 `encode` 编、缓存类型 `cache` 的包查一次：交回 `driver.check` 的回应和查之前、之后的进程号。
async fn checked(encode: &str, cache: &str) -> (Value, Value, Value) {
    let home = Home::new();
    let program = Program::new();
    let how = format!("driver:echo+{encode}");
    install(
        &home,
        &program.name(),
        &steps(&["hello", &how, "serve"]),
        cache,
    );
    let (mut client, core) = connected(&home).await;
    let before = status(&mut client, "ag").await["pid"].clone();
    let reply = client
        .call("k", "driver.check", json!({"package": "ag"}))
        .await;
    let after =
        until_state(&mut client, "ag", |one| one["state"] == "running").await["pid"].clone();
    core.stop_extensions().await;
    (reply, before, after)
}

/// 回应里那一个驱动：结果，和每一份样本的「名字、过没过、为什么」。
fn verdicts(reply: &Value) -> (String, Vec<(String, bool, String)>) {
    let driver = &reply["result"]["drivers"][0];
    let samples = driver["samples"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|sample| {
            (
                sample["sample"].as_str().unwrap_or_default().to_string(),
                sample["ok"].as_bool().unwrap_or_default(),
                sample["why"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    (
        driver["result"].as_str().unwrap_or_default().to_string(),
        samples,
    )
}

#[tokio::test]
async fn a_steady_driver_passes_and_the_extension_was_restarted() {
    let (reply, before, after) = checked("steady", "contract").await;
    let (result, samples) = verdicts(&reply);
    assert_eq!(result, "passed", "{reply}");
    let names: Vec<&str> = samples.iter().map(|(name, _, _)| name.as_str()).collect();
    assert_eq!(
        names,
        ["plain", "tools", "private", "image", "continuation", "pair"]
    );
    assert!(samples.iter().all(|(_, ok, _)| *ok), "{reply}");
    assert_eq!(reply["result"]["drivers"][0]["driver"], "ag/ag");
    assert_eq!(reply["result"]["drivers"][0]["cache"], "contract");
    assert!(
        before.is_u64() && after.is_u64() && before != after,
        "重启过：{before} → {after}"
    );
}

#[tokio::test]
async fn a_timestamp_in_the_body_is_caught_at_once() {
    let (reply, _, _) = checked("stamped", "contract").await;
    let (result, samples) = verdicts(&reply);
    assert_eq!(result, "failed", "{reply}");
    let (name, ok, why) = &samples[0];
    assert_eq!((name.as_str(), *ok), ("plain", false));
    assert!(
        why.starts_with("two encodings of the same request differ at byte "),
        "{why}"
    );
}

#[tokio::test]
async fn changes_after_a_restart_or_before_the_stable_end_are_caught() {
    let (reply, _, _) = checked("pid", "contract").await;
    let (result, samples) = verdicts(&reply);
    assert_eq!(result, "failed", "{reply}");
    assert!(
        samples[0]
            .2
            .starts_with("the encoding changed after a restart at byte "),
        "{reply}"
    );
    let (reply, _, _) = checked("count", "contract").await;
    let (result, samples) = verdicts(&reply);
    assert_eq!(result, "failed", "{reply}");
    let failed: Vec<&str> = samples
        .iter()
        .filter(|(_, ok, _)| !ok)
        .map(|(name, _, _)| name.as_str())
        .collect();
    assert_eq!(failed, ["pair"], "只有那一对：{reply}");
    assert!(
        samples[5].2.contains("before the end of the stable region"),
        "{reply}"
    );
}

#[tokio::test]
async fn a_missing_encode_fails_a_contract_driver_and_skips_others() {
    for (cache, wanted) in [("contract", "failed"), ("best_effort", "skipped")] {
        let (reply, _, _) = checked("none", cache).await;
        let driver = &reply["result"]["drivers"][0];
        assert_eq!(driver["result"], wanted, "{cache}：{reply}");
        assert_eq!(driver["why"], "driver.encode is not implemented");
    }
}

#[tokio::test]
async fn a_driver_in_use_is_not_restarted() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "ag");
    install(
        &home,
        &program.name(),
        &steps(&[&step, "hello", "driver:pair+steady", "serve"]),
        "contract",
    );
    let (mut client, core) = connected(&home).await;
    let before = status(&mut client, "ag").await["pid"].clone();
    // 一个会话正经它调模型：`pair` 压着头一次不答。
    let mut asking = Client::connect(std::sync::Arc::clone(&core));
    asking.hello().await;
    let held = tokio::spawn(async move {
        asking
            .call(
                "m",
                "model.call",
                json!({"purpose": "platform", "model": "g/m", "messages": [{"role": "user", "text": "hi"}]}),
            )
            .await
    });
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    while !read(&path).contains(r#""method":"driver.call""#) {
        assert!(tokio::time::Instant::now() < deadline, "等不到那一次调用");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let reply = client
        .call("k", "driver.check", json!({"package": "ag"}))
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "driver_busy", "{reply}");
    assert_eq!(reply["error"]["data"]["running"], 1, "{reply}");
    assert_eq!(status(&mut client, "ag").await["pid"], before, "没重启");
    core.stop_extensions().await;
    held.abort();
}

#[tokio::test]
async fn a_package_without_drivers_is_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&miyu_session::testkit::Script::new([])));
    client.hello().await;
    let reply = client
        .call("k", "driver.check", json!({"package": "nothing"}))
        .await;
    assert_eq!(
        reply["error"]["data"]["reason"], "unknown_package",
        "{reply}"
    );
}

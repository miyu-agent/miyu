//! 会话的上下文上限在协议上的样子（施工 CTX-1、CTX-2，设计 68 第四节）：`session.configure` 的 `context` 记一条
//! `session.policy_changed`，当场生效：`subscribe` 回应的 `limits.window` 是生效的窗口（模型的和上限里小的，模型没报窗口的
//! 照上限），`limits.context` 是上限；`session.context` 同样带上。太小的 `bad_params`，`data.min` 是最少要多少，什么都不记。
//! `/context` 设、去掉、看，回执照连接的语言。
//!
//! 替身没报最大输出：输出预留照出厂策略的上限 20000，余量 13000 和窗口的 5% 取小，压缩线至多窗口的 85%；算得出压缩线的最小
//! 窗口是 21053。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 照 `params` 发一条 `session.configure`。
async fn configure(client: &mut Client, id: &str, session: &str, context: Value) -> Value {
    let params = json!({"session": session, "context": context});
    client.call(id, "session.configure", params).await
}

/// 这一刻订阅回应里的 `limits`。
async fn limits(client: &mut Client, id: &str, session: &str) -> Value {
    client.subscribe(id, session).await["result"]["limits"].clone()
}

/// 日志里换上限的那几条的 `body`，照先后。
fn caps(home: &Home, session: &str) -> Vec<Value> {
    logged(home, session)
        .into_iter()
        .filter(|event| {
            event["kind"] == "session.policy_changed" && event["body"]["context"].is_object()
        })
        .map(|event| event["body"].clone())
        .collect()
}

/// 设了取小的、当场生效；一样的不记；`null` 去掉；记的那一条带着命令的编号。
#[tokio::test]
async fn the_cap_takes_the_smaller_window_at_once() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([]).window(1_000_000)));
    client.hello().await;
    let session = client.create("c1", "~").await;
    assert_eq!(
        limits(&mut client, "c2", &session).await,
        json!({"window": 1_000_000, "compaction_line": 850_000})
    );
    let reply = configure(&mut client, "c3", &session, json!(128_000)).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(
        limits(&mut client, "c4", &session).await,
        json!({"window": 128_000, "compaction_line": 101_600, "context": 128_000}),
        "没开回合就换上：预留 20000、余量 6400"
    );
    let reply = configure(&mut client, "c5", &session, json!(128_000)).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let reply = configure(&mut client, "c6", &session, json!(null)).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(
        limits(&mut client, "c7", &session).await,
        json!({"window": 1_000_000, "compaction_line": 850_000})
    );
    assert_eq!(
        caps(&home, &session),
        [
            json!({"context": {"tokens": 128_000}}),
            json!({"context": {"tokens": null}})
        ],
        "一样的不记"
    );
    let cause = logged(&home, &session)
        .into_iter()
        .find(|event| event["body"]["context"].is_object())
        .map(|event| event["cause"].clone());
    assert_eq!(cause, Some(json!("c3/context")));
}

/// 模型的窗口比上限小的照模型的；模型没报窗口的照上限。`session.context` 也带上限。
#[tokio::test]
async fn a_smaller_model_window_wins_and_no_window_takes_the_cap() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([]).window(64_000)));
    client.hello().await;
    let session = client.create("c1", "~").await;
    configure(&mut client, "c2", &session, json!(128_000)).await;
    assert_eq!(
        limits(&mut client, "c3", &session).await,
        json!({"window": 64_000, "compaction_line": 40_800, "context": 128_000}),
        "模型的小：预留 20000、余量 3200"
    );
    let detail = client
        .call("c4", "session.context", json!({"session": session}))
        .await;
    assert_eq!(
        (
            &detail["result"]["window"],
            &detail["result"]["line"],
            &detail["result"]["context"]
        ),
        (&json!(64_000), &json!(40_800), &json!(128_000)),
        "{detail}"
    );

    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    configure(&mut client, "c2", &session, json!(128_000)).await;
    assert_eq!(
        limits(&mut client, "c3", &session).await,
        json!({"window": 128_000, "compaction_line": 101_600, "context": 128_000}),
        "模型没报窗口：照上限"
    );
}

/// 太小、0 的 `bad_params`，`data.field` 是 `context`、`data.min` 是最少要多少，什么都不记；不是整数的参数不对。
#[tokio::test]
async fn too_small_a_cap_is_refused_with_the_least() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([]).window(1_000_000)));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for (id, tokens) in [("c2", 21_052), ("c3", 0)] {
        let reply = configure(&mut client, id, &session, json!(tokens)).await;
        assert_eq!(reply["error"]["data"]["reason"], "bad_params", "{reply}");
        assert_eq!(reply["error"]["data"]["field"], "context", "{reply}");
        assert_eq!(reply["error"]["data"]["min"], 21_053, "{reply}");
    }
    for (id, wrong) in [("c4", json!("128k")), ("c5", json!(-1)), ("c6", json!(1.5))] {
        let reply = configure(&mut client, id, &session, wrong).await;
        assert_eq!(reply["error"]["data"]["reason"], "bad_params", "{reply}");
    }
    let reply = configure(&mut client, "c7", &session, json!(21_053)).await;
    assert_eq!(reply["result"], json!({}), "正好够的收：{reply}");
    assert_eq!(caps(&home, &session).len(), 1, "拒了的什么都不记");
}

/// 在会话 `session` 里打 `text`。
async fn run(client: &mut Client, session: &str, id: &str, text: &str) -> Value {
    let params = json!({"session": session, "text": text});
    client.call(id, "command.run", params).await
}

/// `/context`：数值、`k`、`m`、`off`、不带参数；写错的参数不对，太小的照 `session.configure` 拒。
#[tokio::test]
async fn the_context_command_sets_shows_and_takes_off() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([]).window(1_000_000)));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let said = |reply: &Value| reply["result"]["said"].clone();
    let shown = run(&mut client, &session, "k1", "/context").await;
    assert_eq!(said(&shown), "未设上下文上限，压缩线约 850k。", "{shown}");
    assert_eq!(
        shown["result"]["events"].as_array().map(Vec::len),
        Some(1),
        "只记 command.ran"
    );
    let set = run(&mut client, &session, "k2", "/context 128K").await;
    assert_eq!(
        said(&set),
        "上下文上限设为约 128k，压缩线约 101k。",
        "{set}"
    );
    assert_eq!(
        said(&run(&mut client, &session, "k3", "/context").await),
        "上下文上限约 128k，压缩线约 101k。"
    );
    assert_eq!(
        said(&run(&mut client, &session, "k4", "/context 200000").await),
        "上下文上限设为约 200k，压缩线约 170k。"
    );
    assert_eq!(
        said(&run(&mut client, &session, "k5", "/context 1m").await),
        "上下文上限设为约 1m，压缩线约 850k。"
    );
    assert_eq!(
        said(&run(&mut client, &session, "k6", "/context off").await),
        "已去掉上下文上限，压缩线约 850k。"
    );
    for (id, wrong) in [
        ("k7", "/context abc"),
        ("k8", "/context 1.5m"),
        ("k9", "/context 12 k"),
    ] {
        let reply = run(&mut client, &session, id, wrong).await;
        assert_eq!(
            reply["error"]["data"]["reason"], "bad_params",
            "{wrong}：{reply}"
        );
    }
    let small = run(&mut client, &session, "k10", "/context 1k").await;
    assert_eq!(small["error"]["data"]["field"], "context", "{small}");
    assert_eq!(small["error"]["data"]["min"], 21_053, "{small}");
    assert_eq!(
        caps(&home, &session),
        [
            json!({"context": {"tokens": 128_000}}),
            json!({"context": {"tokens": 200_000}}),
            json!({"context": {"tokens": 1_000_000}}),
            json!({"context": {"tokens": null}})
        ]
    );
}

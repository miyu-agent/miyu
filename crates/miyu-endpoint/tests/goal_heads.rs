//! 两个头看长期目标（施工 X-8 补）：真核心、假模型。事件流订阅的回应带 `goal`（同会话状态那一格），变了推瞬时事件
//! `goal.changed`；续轮那条 `message.user` 的 `cause` 照约定写着第几轮；视图条目带 `round`。

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::configured_core;
use crate::support::*;

fn goal_tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = miyu_goal::tools(&resources).expect("读得出");
    Catalog::in_packages([(miyu_goal::PACKAGE, tools)]).expect("合写法")
}

/// 发一条请求，交回回应之前读到的推送和回应（订阅着的会话，推送和回应交错）。
async fn request(
    client: &mut Client,
    id: &str,
    method: &str,
    params: Value,
) -> (Vec<Value>, Value) {
    let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    client.line(&line.to_string()).await;
    client.until_reply(id).await
}

/// 推送里的 `goal.changed`：交回 `(state, round)`。
fn changes(pushed: &[Value]) -> Vec<(String, u64)> {
    pushed
        .iter()
        .filter(|push| push["params"]["event"]["kind"] == "goal.changed")
        .map(|push| {
            let goal = &push["params"]["event"]["body"]["goal"];
            (
                goal["state"].as_str().unwrap_or_default().to_string(),
                goal["round"].as_u64().unwrap_or_default(),
            )
        })
        .collect()
}

#[tokio::test]
async fn heads_see_the_goal_without_working_it_out() {
    let home = Home::new();
    let script = Script::new([Play::Says("在等你确认。")]).titles([Play::Says("测试")]);
    let mut client = Client::connect(configured_core(&home, &script, goal_tools()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let subscribed = client.subscribe("s1", &session).await;
    assert!(
        subscribed["result"].get("goal").is_none(),
        "没有目标的不写：{subscribed}"
    );

    let (mut pushed, reply) = request(
        &mut client,
        "k1",
        "command.run",
        json!({"session": session, "text": "/goal --rounds 3 把测试修绿"}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "goal", "{reply}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !changes(&pushed).iter().any(|(state, _)| state == "waiting") {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        pushed.push(client.next_within(left).await.expect("六十秒内等到等人"));
    }
    let mut seen = changes(&pushed);
    seen.dedup();
    assert_eq!(
        seen,
        [
            ("running".to_string(), 0),
            ("running".to_string(), 1),
            ("waiting".to_string(), 1)
        ]
    );
    let changed = pushed
        .iter()
        .find(|push| push["params"]["event"]["kind"] == "goal.changed")
        .expect("推了");
    assert_eq!(
        changed["params"]["event"]["by"],
        json!({"kind": "module", "id": "goal"})
    );
    assert_eq!(changed["params"]["event"]["body"]["goal"]["limit"], 3);
    let continuation = pushed
        .iter()
        .map(|push| &push["params"]["event"])
        .find(|event| event["kind"] == "message.user" && event["by"]["kind"] == "module")
        .expect("有续轮消息");
    assert!(
        continuation["cause"]
            .as_str()
            .is_some_and(|cause| cause.starts_with("goal-1-")),
        "{continuation}"
    );

    // 还是那一个订阅、再订阅一次：这一刻的目标在回应里。
    let (_, again) = request(
        &mut client,
        "s2",
        "subscribe",
        json!({"session": session, "stream": "events"}),
    )
    .await;
    let goal = &again["result"]["goal"];
    assert_eq!(
        (
            &goal["text"],
            &goal["state"],
            &goal["round"],
            &goal["limit"]
        ),
        (
            &json!("把测试修绿"),
            &json!("waiting"),
            &json!(1),
            &json!(3)
        ),
        "{again}"
    );

    let (_, page) = request(
        &mut client,
        "p1",
        "view.page",
        json!({"session": session, "view": true}),
    )
    .await;
    let round = page["result"]["entries"]
        .as_array()
        .expect("有条目")
        .iter()
        .find(|entry| entry["kind"] == "user" && entry["from"]["kind"] == "module")
        .map(|entry| entry["round"].clone());
    assert_eq!(round, Some(json!(1)), "{page}");
}

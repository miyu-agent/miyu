//! 建索引的进度推给头（施工 W-15，网页的知识库页）：订阅 `kb` 流，回应是空的；重建索引时推 `kb.indexing`（开头 0，最后一篇照推）
//! 和 `kb.indexed`；`kb.list` 每个库带 `indexing`、建好过的带 `indexed_at`。

use serde_json::{Value, json};

use crate::knowledge_manage::{client, upload};
use crate::support::*;

/// 一直读，直到 `kb.indexed` 和重建的回应（编号 `r`）都到了：交回读到的进度推送（`[done, total]`）和回应。推送经订阅的转发
/// 任务写出去，回应由处理请求的那一头写，两条路谁先到不一定：macOS 的 CI 上回应先到过，原来读推送时跳过了它，再等回应就等满
/// 六十秒（W-15 修）。
async fn progress_and_answer(client: &mut Client) -> (Vec<(u64, u64)>, Value) {
    let (mut seen, mut indexed, mut answer) = (Vec::new(), false, None);
    while !indexed || answer.is_none() {
        let next: Value = client.next().await.expect("没断开");
        if next["id"] == "r" {
            answer = Some(next);
            continue;
        }
        match next["method"].as_str() {
            Some("kb.indexing") => {
                assert!(!indexed, "建好了不再推进度：{next}");
                assert_eq!(next["params"]["kb"], "own", "{next}");
                let at = |field: &str| next["params"][field].as_u64().expect("是数");
                seen.push((at("done"), at("total")));
            }
            Some("kb.indexed") => {
                assert_eq!(next["params"], json!({"kb": "own"}), "{next}");
                indexed = true;
            }
            _ => {}
        }
    }
    (seen, answer.expect("等到了回应"))
}

#[tokio::test]
async fn rebuilding_pushes_progress_and_the_list_says_when() {
    let home = Home::new();
    let mut client = client(&home).await;
    client.call("c", "kb.create", json!({"kb": "own"})).await;
    for (n, name) in ["a.md", "b.md", "c.md"].into_iter().enumerate() {
        let blob = upload(&mut client, &format!("b{n}"), name, "猫\n").await;
        client
            .call(
                &format!("a{n}"),
                "kb.add",
                json!({"kb": "own", "path": name, "blob": blob}),
            )
            .await;
    }
    let subscribed = client.call("s", "subscribe", json!({"stream": "kb"})).await;
    assert_eq!(subscribed["result"], json!({}), "{subscribed}");
    let request =
        json!({"jsonrpc": "2.0", "id": "r", "method": "kb.reindex", "params": {"kb": "own"}});
    client.line(&request.to_string()).await;
    let (seen, answer) = progress_and_answer(&mut client).await;
    assert_eq!(seen.first(), Some(&(0, 3)), "{seen:?}");
    assert_eq!(seen.last(), Some(&(3, 3)), "最后一篇照推：{seen:?}");
    assert_eq!(
        answer["result"]["documents"],
        json!(3),
        "补完了才回：{answer}"
    );

    let listed = client.call("l", "kb.list", json!({})).await;
    let base = &listed["result"]["bases"][0];
    assert_eq!(base["indexing"], json!(false), "{listed}");
    assert!(
        base["indexed_at"]
            .as_str()
            .is_some_and(|at| at.ends_with('Z')),
        "{listed}"
    );
    let gone = client
        .call("u", "unsubscribe", json!({"stream": "kb"}))
        .await;
    assert_eq!(gone["result"], json!({}), "{gone}");
}

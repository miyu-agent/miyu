//! 建索引的进度推给头（施工 W-15，网页的知识库页）：订阅 `kb` 流，回应是空的；重建索引时推 `kb.indexing`（开头 0，最后一篇照推）
//! 和 `kb.indexed`；`kb.list` 每个库带 `indexing`、建好过的带 `indexed_at`。

use serde_json::{Value, json};

use crate::knowledge_manage::{client, upload};
use crate::support::*;

/// 一直读推送，直到 `kb.indexed`：交回读到的进度推送（`[done, total]`）。
async fn progress_until_indexed(client: &mut Client) -> Vec<(u64, u64)> {
    let mut seen = Vec::new();
    loop {
        let next: Value = client.next().await.expect("没断开");
        match next["method"].as_str() {
            Some("kb.indexing") => {
                assert_eq!(next["params"]["kb"], "own", "{next}");
                let at = |field: &str| next["params"][field].as_u64().expect("是数");
                seen.push((at("done"), at("total")));
            }
            Some("kb.indexed") => {
                assert_eq!(next["params"], json!({"kb": "own"}), "{next}");
                return seen;
            }
            _ => {}
        }
    }
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
    let seen = progress_until_indexed(&mut client).await;
    assert_eq!(seen.first(), Some(&(0, 3)), "{seen:?}");
    assert_eq!(seen.last(), Some(&(3, 3)), "最后一篇照推：{seen:?}");
    // 重建的回应在推送后面到（补完了才回）。
    loop {
        let next = client.next().await.expect("没断开");
        if next["id"] == "r" {
            assert_eq!(next["result"]["documents"], json!(3), "{next}");
            break;
        }
    }

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

//! 有没有能压的（施工 W-12，网页要的：换人格、预设之前问不问「切换前会压缩」）：订阅的回应、视图状态都带 `compactable`，
//! 和 `session.compact` 回不回 `nothing_to_compact` 一个判法；说了一大段以后变成能压，视图流跟着推，事件流推瞬时的
//! `compactable.changed`（施工 W-12 补，终端要的）。

use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::{Play, Script};

/// 窗口大到不会自动压；出厂的尾巴是 16000 token，说得短的全在尾巴里，说七万个字的一组就超了尾巴（同 `compact.rs`）。
fn script() -> Script {
    Script::new([Play::Says("好。")]).window(1_000_000)
}

/// 一直读推送，读到一份 `compactable` 是 `want` 的 `view.status`。
async fn status_until(client: &mut Client, want: bool) -> Value {
    loop {
        let next = client.next().await.expect("没断开");
        if next["method"] == "view.status" && next["params"]["status"]["compactable"] == json!(want)
        {
            return next["params"]["status"].clone();
        }
    }
}

#[tokio::test]
async fn compactable_follows_what_session_compact_would_say() {
    let home = Home::new();
    let core = home.core(&script());
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    // 事件流另开一个连接订（同一个连接对一个会话只有一个订阅，`view` 也算）：变了推瞬时的 `compactable.changed`（施工 W-12 补）。
    let mut events = Client::connect(core);
    events.hello().await;
    events.subscribe("e1", &session).await;
    let reply = client.subscribe("c2", &session).await;
    assert_eq!(
        reply["result"]["compactable"],
        json!(false),
        "新会话没有能压的：{reply}"
    );
    let view = client
        .call(
            "v1",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    assert_eq!(
        view["result"]["status"]["compactable"],
        json!(false),
        "{view}"
    );

    client.say("c3", &session, &"x".repeat(70_000)).await;
    home.until_turns(&session, 1).await;
    let status = status_until(&mut client, true).await;
    assert_eq!(status["compactable"], json!(true));
    let reply = client.subscribe("c4", &session).await;
    assert_eq!(reply["result"]["compactable"], json!(true), "{reply}");
    loop {
        let next = events.next().await.expect("没断开");
        let event = &next["params"]["event"];
        if event["kind"] == "compactable.changed" {
            assert_eq!(event["body"], json!({"compactable": true}), "{next}");
            assert_eq!(event["by"], json!({"kind": "kernel"}), "{next}");
            break;
        }
    }
}

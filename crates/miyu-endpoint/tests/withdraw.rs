//! 不打断，退回一条排着的（施工 Q-W，`session.withdraw`）：真的核心，回合停在请求上，排两句，先退最新的、再点名退另一句；
//! 这一轮一直在跑，都退完了打断「接着发」也不再开一轮。不在队里的 `already_heard`，没有排着的 `nothing_queued`。

use serde_json::{Value, json};

use crate::support::{Client, Home, reason, until};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};

/// 说一句，交回那条 `message.user` 的序号。
async fn queued(client: &mut Client, id: &str, session: &str, text: &str) -> u64 {
    let reply = client.say(id, session, text).await;
    reply["result"]["events"][0].as_u64().expect("记下了那句话")
}

async fn withdraw(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "session.withdraw", params).await
}

#[tokio::test]
async fn queued_messages_are_taken_back_one_at_a_time() {
    let home = Home::new();
    let script = Script::new([Play::Holds, Play::Says("二。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let first = queued(&mut client, "c2", &session, "one").await;
    until("请求停住", || script.requests().len() == 1).await;
    let two = queued(&mut client, "c3", &session, "two").await;
    let three = queued(&mut client, "c4", &session, "three").await;

    let reply = withdraw(&mut client, "c5", json!({"session": session})).await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    let reply = withdraw(
        &mut client,
        "c6",
        json!({"session": session, "message": two}),
    )
    .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    let withdrawn: Vec<Vec<u64>> = home
        .log(&session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageWithdrawn(withdrawn) => {
                Some(withdrawn.messages.iter().map(|seq| seq.get()).collect())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        withdrawn,
        vec![vec![three], vec![two]],
        "先退最新的，再退点名的"
    );
    assert!(
        !home
            .log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_))),
        "这一轮还在跑"
    );

    // 开这一轮的那一句、退过的：她听到了，或者已经不在队里。
    for (id, message) in [("c7", first), ("c8", three)] {
        let reply = withdraw(
            &mut client,
            id,
            json!({"session": session, "message": message}),
        )
        .await;
        assert_eq!(reason(&reply), Some("already_heard"), "{reply}");
    }
    let reply = withdraw(&mut client, "c9", json!({"session": session})).await;
    assert_eq!(reason(&reply), Some("nothing_queued"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("没有排队的消息。"));

    // 都退完了：打断时「接着发」也没有能发的，不再开一轮。
    let stop = json!({"session": session, "queued": "send"});
    client.call("c10", "session.interrupt", stop).await;
    home.until_turns(&session, 1).await;
    let started = home
        .log(&session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count();
    assert_eq!(started, 1, "不再开一轮");
    assert_eq!(script.requests().len(), 1);
}

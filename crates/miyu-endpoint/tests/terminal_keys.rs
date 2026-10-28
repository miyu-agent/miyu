//! 按终端分会话（施工 T-22，设计 `22-命令行.md` 第七节）：带 `terminal` 键的照「账号 + 键」各记一个指针，不带的照旧用原来那一个；
//! `create: false` 只问不开。

use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::Script;

/// 要终端会话，交回整个回应。
async fn terminal(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "session.terminal", params).await
}

/// 会话列表里有几个会话。
async fn count(client: &mut Client, id: &str) -> usize {
    let listed = client.call(id, "session.list", json!({})).await;
    listed["result"]["sessions"].as_array().map_or(0, Vec::len)
}

#[tokio::test]
async fn each_terminal_key_has_its_own_session() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let a = terminal(&mut client, "a1", json!({"terminal": "pane-a"})).await;
    let b = terminal(&mut client, "b1", json!({"terminal": "pane-b"})).await;
    assert_eq!(a["result"]["created"], json!(true), "{a}");
    assert_eq!(b["result"]["created"], json!(true), "{b}");
    assert_ne!(
        a["result"]["session"], b["result"]["session"],
        "两个终端两个会话"
    );
    // 各自再要的还是各自的；不带键的是原来那一个指针，和两个都不相干。
    let again = terminal(&mut client, "a2", json!({"terminal": "pane-a"})).await;
    assert_eq!(again["result"], json!({"session": a["result"]["session"]}));
    let main = terminal(&mut client, "m1", json!({})).await;
    assert_eq!(main["result"]["created"], json!(true), "{main}");
    assert_ne!(main["result"]["session"], a["result"]["session"]);
    assert_ne!(main["result"]["session"], b["result"]["session"]);

    // 改一个键的指针不碰别的。
    let pointed = client
        .call(
            "p1",
            "session.set_terminal",
            json!({"session": b["result"]["session"], "terminal": "pane-a"}),
        )
        .await;
    assert_eq!(
        pointed["result"],
        json!({"session": b["result"]["session"]}),
        "{pointed}"
    );
    let a_now = terminal(&mut client, "a3", json!({"terminal": "pane-a"})).await;
    assert_eq!(a_now["result"], json!({"session": b["result"]["session"]}));
    let main_now = terminal(&mut client, "m2", json!({})).await;
    assert_eq!(
        main_now["result"],
        json!({"session": main["result"]["session"]})
    );
}

#[tokio::test]
async fn asking_without_create_never_opens_one() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let asked = terminal(
        &mut client,
        "q1",
        json!({"terminal": "pane-a", "create": false}),
    )
    .await;
    assert_eq!(asked["result"], json!({"session": null}), "{asked}");
    let asked_main = terminal(&mut client, "q2", json!({"create": false})).await;
    assert_eq!(
        asked_main["result"],
        json!({"session": null}),
        "{asked_main}"
    );
    assert_eq!(count(&mut client, "l1").await, 0, "没开会话");
    assert!(
        !home.root.path().join("terminal.json").exists(),
        "也没记指针"
    );

    // 有了以后只问也交回它。
    let made = terminal(&mut client, "t1", json!({"terminal": "pane-a"})).await;
    let session = made["result"]["session"].clone();
    let asked = terminal(
        &mut client,
        "q3",
        json!({"terminal": "pane-a", "create": false}),
    )
    .await;
    assert_eq!(asked["result"], json!({"session": session}), "{asked}");

    // 指着的删了：只问的回没有，不新开。
    client
        .call("d1", "session.delete", json!({"session": session}))
        .await;
    let gone = terminal(
        &mut client,
        "q4",
        json!({"terminal": "pane-a", "create": false}),
    )
    .await;
    assert_eq!(gone["result"], json!({"session": null}), "{gone}");
    assert_eq!(count(&mut client, "l2").await, 0);
}

#[tokio::test]
async fn bad_terminal_keys_and_fresh_without_create_are_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let long = "k".repeat(129);
    for (id, params) in [
        ("e1", json!({"terminal": ""})),
        ("e2", json!({"terminal": long})),
        ("e3", json!({"terminal": "a\nb"})),
        ("e4", json!({"fresh": true, "create": false})),
    ] {
        let refused = terminal(&mut client, id, params).await;
        assert_eq!(reason(&refused), Some("bad_params"), "{refused}");
    }
    let made = terminal(&mut client, "t1", json!({})).await;
    let set = client
        .call(
            "s1",
            "session.set_terminal",
            json!({"session": made["result"]["session"], "terminal": ""}),
        )
        .await;
    assert_eq!(reason(&set), Some("bad_params"), "{set}");
    // 128 个字的收。
    let edge = terminal(&mut client, "t2", json!({"terminal": "k".repeat(128)})).await;
    assert_eq!(edge["result"]["created"], json!(true), "{edge}");
    assert_eq!(count(&mut client, "l1").await, 2, "被拒的都没开会话");
}

#[tokio::test]
async fn writing_a_pointer_drops_keys_unused_for_a_week() {
    let home = Home::new();
    let dir = home.root.path().to_path_buf();
    std::fs::create_dir_all(&dir).expect("建得了目录");
    let stale = r#"{"terminals":{"old-pane":{"session":"01a0f233-cfec-7023-8ed5-2a037a1d5ec8","at":"2026-01-01T00:00:00.000Z","used":"2026-01-01T00:00:00.000Z"}}}"#;
    std::fs::write(dir.join("terminal.json"), stale).expect("写得进");
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let made = terminal(&mut client, "t1", json!({"terminal": "new-pane"})).await;
    assert_eq!(made["result"]["created"], json!(true), "{made}");
    let written = std::fs::read_to_string(dir.join("terminal.json")).expect("记下了");
    assert!(written.contains("new-pane"), "{written}");
    assert!(!written.contains("old-pane"), "7 天没用的清掉了：{written}");
}

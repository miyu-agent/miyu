//! `miyu ask` 掉队以后接着跟（发布前第三轮审计 R3E-3）：收到 `resync` 重新订阅时带上看到的最后一个序号（`after`），核心把
//! 掉的那一截补回来，这一轮的 `turn.ended` 照样等得到。一个假的核心：说话以后推这一轮的开头和回复，接着推 `resync`、
//! 不推 `turn.ended`；只有带 `after` 的订阅才把掉的那一截补回来。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::support::{Asked, Bare, ask_at, dirs, plan};
use miyu_ipc::Listener;

/// 一条落了盘的推送：会话 `s1` 的第 `seq` 条，由命令 `cause` 引起。
fn event(seq: u64, kind: &str, cause: &Value, body: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": "event", "params": {"session": "s1", "event": {
        "seq": seq, "kind": kind, "turn": 2, "cause": cause, "body": body}}})
}

/// 假的核心：记下收到的每一次 `subscribe` 的参数。第一轮的 `turn.ended`（第 4 条）掉在 `resync` 那一截里，只有带 `after`
/// 的重新订阅补得回来；不带的照真核心那样只接以后的，这一轮就永远等不到结束。
async fn drops_the_end(mut listener: Listener, subscribed: Arc<Mutex<Vec<Value>>>) {
    let Ok(connection) = listener.accept().await else {
        return;
    };
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read).lines();
    // 落了盘的那几条：说话以后才有。
    let mut log: Vec<Value> = Vec::new();
    while let Ok(Some(line)) = lines.next_line().await {
        let request: Value = serde_json::from_str(&line).expect("是 JSON");
        let id = request["id"].clone();
        let mut out = Vec::new();
        let result = match request["method"].as_str() {
            Some("hello") => json!({"protocol": 1, "core": {"version": "0"}}),
            Some("session.create") => json!({"session": "s1", "events": [1]}),
            Some("subscribe") => {
                subscribed
                    .lock()
                    .expect("没 panic")
                    .push(request["params"].clone());
                match request["params"]["after"].as_u64() {
                    Some(after) => {
                        let missed = log
                            .iter()
                            .filter(|line| line["params"]["event"]["seq"].as_u64() > Some(after));
                        out.extend(missed.cloned());
                        json!({"upto": 4})
                    }
                    None => json!({}),
                }
            }
            Some("session.send") => {
                log = vec![
                    event(2, "turn.started", &id, json!({"trigger": 2})),
                    event(
                        3,
                        "message.assistant",
                        &id,
                        json!({"blocks": [{"type": "text", "text": "好。"}]}),
                    ),
                    event(4, "turn.ended", &id, json!({"reason": "completed"})),
                ];
                // 回答的增量是瞬时的，没有序号：夹在回合开头和那条回复中间。
                let delta = |body: Value| {
                    json!({"jsonrpc": "2.0", "method": "event", "params": {"session": "s1",
                        "event": {"kind": "model.delta", "turn": 2, "body": body}}})
                };
                out.extend([
                    log[0].clone(),
                    delta(json!({"index": 0, "start": "text"})),
                    delta(json!({"index": 0, "text": "好。"})),
                    log[1].clone(),
                ]);
                out.push(json!({"jsonrpc": "2.0", "method": "resync",
                    "params": {"session": "s1", "stream": "events"}}));
                json!({"events": [2]})
            }
            _ => json!({}),
        };
        out.push(json!({"jsonrpc": "2.0", "id": id, "result": result}));
        for message in out {
            let line = format!("{message}\n");
            if write.write_all(line.as_bytes()).await.is_err() {
                return;
            }
        }
    }
}

#[tokio::test]
async fn after_a_resync_the_missed_end_of_the_turn_is_replayed() {
    let bare = Bare::new();
    let opened = miyu_ipc::open(&bare.root, &dirs()).expect("起得来");
    let subscribed = Arc::new(Mutex::new(Vec::new()));
    let core = tokio::spawn(drops_the_end(opened.listener, Arc::clone(&subscribed)));
    let (_press, presses) = mpsc::channel(1);
    // 掉了 `turn.ended` 又补不回来的，没写 `--timeout` 就一直等：十秒没说完就是等不到了。
    let asked = tokio::time::timeout(
        Duration::from_secs(10),
        ask_at(&bare.root, &plan("在吗"), presses),
    )
    .await;
    core.abort();
    let subscribed = subscribed.lock().expect("没 panic").clone();
    let Ok(Asked { code, out, .. }) = asked else {
        panic!("掉队以后等不到这一轮结束；订阅过的参数：{subscribed:?}");
    };
    assert_eq!(code, 0);
    assert_eq!(out, "好。\n");
    assert_eq!(subscribed.len(), 2, "订阅一次、掉队以后重新订阅一次");
    assert_eq!(
        subscribed[1]["after"],
        json!(3),
        "从看到的最后一条以后接着补"
    );
}

//! 通知的参数、每个扩展的队列（施工 HK-2）：满了丢新的、照扩展累计；没在跑的丢掉队列；连接换了新开一条。

use serde_json::json;
use tokio::sync::mpsc;

use super::*;

/// 一条直接调的通知。
fn direct() -> AfterNote {
    AfterNote {
        session: "s".to_string(),
        venue: "local".to_string(),
        confined: false,
        call_id: "call_3_1".to_string(),
        tool: "shell".to_string(),
        access: Some("execute".to_string()),
        status: "ok".to_string(),
        duration_ms: Some(12),
        by: json!({"kind": "tool", "call_id": "call_3_1"}),
        args: "{\"command\":\"ls\"}".to_string(),
        outcome: AfterOutcome::Blocks(json!([{"type": "text", "text": "a"}])),
    }
}

#[test]
fn a_direct_call_carries_blocks_and_args_as_written() {
    assert_eq!(
        params_of(&direct()),
        json!({
            "session": "s", "call_id": "call_3_1", "tool": "shell", "access": "execute", "status": "ok",
            "duration_ms": 12, "by": {"kind": "tool", "call_id": "call_3_1"}, "args": "{\"command\":\"ls\"}",
            "blocks": [{"type": "text", "text": "a"}],
        })
    );
}

#[test]
fn a_nested_call_carries_the_blob_and_leaves_out_what_it_lacks() {
    let mut note = direct();
    note.call_id = "call_3_1#2".to_string();
    note.access = None;
    note.duration_ms = None;
    note.status = "needs_approval".to_string();
    note.outcome = AfterOutcome::Stored {
        result: None,
        bytes: None,
    };
    let params = params_of(&note);
    for key in ["blocks", "result", "bytes", "access", "duration_ms"] {
        assert!(params.get(key).is_none(), "{key}");
    }
    note.outcome = AfterOutcome::Stored {
        result: Some("ab".repeat(32)),
        bytes: Some(7),
    };
    let params = params_of(&note);
    assert_eq!(params["result"], json!("ab".repeat(32)));
    assert_eq!(params["bytes"], json!(7));
}

/// 写队列里的下一行：等一会儿还没有的算没发到这个连接。
async fn next(written: &mut mpsc::Receiver<String>) -> String {
    tokio::time::timeout(std::time::Duration::from_secs(5), written.recv())
        .await
        .expect("这个连接收到了")
        .expect("连接还在")
}

#[tokio::test(flavor = "current_thread")]
async fn a_full_queue_drops_new_ones_and_counts_them() {
    let afters = Afters::default();
    // 写队列只放得下一条、没人读：发送任务拿走一条就卡住，后面的都攒在通知的队列里。
    let (out, mut written) = mpsc::channel(1);
    let peer = Peer::new(out);
    // 设计 65 第四节第 4 条：256 条。
    let total = 256 + 40;
    for k in 0..total {
        afters.put("audit", Some(peer.clone()), &json!({"k": k}));
    }
    {
        let outlets = afters.outlets.lock().unwrap();
        let outlet = &outlets["audit"];
        assert_eq!(outlet.dropped, 40, "满了丢新的，照扩展累计");
        assert!(outlet.dropping);
    }
    // 读走的照先后：头一条是第 0 条，最后一条是第 255 条（丢的是新的）。
    let mut seen = Vec::new();
    while seen.len() < 256 {
        let line: Value = serde_json::from_str(&next(&mut written).await).unwrap();
        assert_eq!(line["method"], "tool.after");
        seen.push(line["params"]["k"].as_u64().unwrap());
    }
    let expected: Vec<u64> = (0..256).collect();
    assert_eq!(seen, expected);
    // 有了空位接着收，累计的数留着。
    afters.put("audit", Some(peer.clone()), &json!({"k": "later"}));
    let outlets = afters.outlets.lock().unwrap();
    assert!(!outlets["audit"].dropping);
    assert_eq!(outlets["audit"].dropped, 40);
}

#[tokio::test(flavor = "current_thread")]
async fn a_stopped_extension_loses_its_queue_and_a_restart_starts_empty() {
    let afters = Afters::default();
    let (out, _written) = mpsc::channel(1);
    let first = Peer::new(out);
    for k in 0..256 + 5 {
        afters.put("audit", Some(first.clone()), &json!({"k": k}));
    }
    assert_eq!(afters.outlets.lock().unwrap()["audit"].dropped, 5);
    // 没在跑：不发，队列丢掉。
    afters.put("audit", None, &json!({"k": "gone"}));
    assert!(afters.outlets.lock().unwrap().get("audit").is_none());
    // 重新握手的连接：新开一条空的，不补以前的。
    let (out, mut written) = mpsc::channel(4);
    let second = Peer::new(out);
    afters.put("audit", Some(second.clone()), &json!({"k": "new"}));
    let line: Value = serde_json::from_str(&next(&mut written).await).unwrap();
    assert_eq!(line["params"]["k"], "new");
    assert_eq!(afters.outlets.lock().unwrap()["audit"].dropped, 0);
    // 连接换了（没先停）的同样新开。
    let (out, mut third_written) = mpsc::channel(4);
    afters.put("audit", Some(Peer::new(out)), &json!({"k": "third"}));
    let line: Value = serde_json::from_str(&next(&mut third_written).await).unwrap();
    assert_eq!(line["params"]["k"], "third");
}

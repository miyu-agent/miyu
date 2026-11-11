//! 用户态总线（施工 BUS-2，设计 `67-总线.md` 第四节）：真核心、测试用的扩展。`bus.list` 列出装着的包在总线上的名字，`ready`
//! 照登记算；`bus.call` 照名字找到包、拼上前缀转过去：`provides` 的一家直接调，`plugs` 的要写挑哪家；短名带点的
//! `bad_params`，没人提供的 `no_provider`，没导出的 `not_exported`；扩展回的错的码照 `error.data.code` 交回。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::extensions::{Program, core, install, quick, record, until_state};
use crate::support::*;

/// 装一个扩展 `id`：登记 `methods`（全名）、导出 `export`（`[export]` 那几行），起来以后照登记的答。交回记下的文件。
fn voice(
    home: &Home,
    program: &Program,
    id: &str,
    methods: &[&str],
    export: &str,
) -> std::path::PathBuf {
    let (path, keep) = record(home, id);
    let listed: Vec<String> = methods
        .iter()
        .map(|name| format!(r#"{{"name":"{name}"}}"#))
        .collect();
    let register = format!(
        r#"ask:package.methods:{{"methods":[{}]}}"#,
        listed.join(",")
    );
    let args: Vec<String> = [keep.as_str(), "hello", register.as_str(), "serve"]
        .iter()
        .map(ToString::to_string)
        .collect();
    install(home, id, &program.name(), "manual", &args);
    let manifest = home.root.path().join(format!("opt/{id}/package.toml"));
    let text = std::fs::read_to_string(&manifest).expect("装好了");
    std::fs::write(&manifest, format!("{text}\n[export]\n{export}\n")).expect("写得进");
    path
}

/// 开起来、等它登记了 `count` 个方法。
async fn started(client: &mut Client, id: &str, path: &std::path::Path, count: usize) {
    let on = client
        .call("e", "package.enable", json!({"package": id}))
        .await;
    assert!(on.get("error").is_none(), "{on}");
    until_state(client, id, |one| one["state"] == "running").await;
    let wanted = format!(r#""methods":{count}"#);
    let registered = tokio::time::timeout(Duration::from_secs(60), async {
        while !std::fs::read_to_string(path)
            .unwrap_or_default()
            .contains(&wanted)
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(registered.is_ok(), "{id} 登记了 {count} 个方法");
}

async fn bus(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "bus.call", params).await
}

fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

#[tokio::test]
async fn names_are_listed_and_called_without_package_ids() {
    let home = Home::new();
    let program = Program::new();
    let stt = voice(
        &home,
        &program,
        "voice-in",
        &["stt.transcribe", "stt.coded"],
        "provides = [\"stt\"]\nmethods = [\"stt.transcribe\", \"stt.coded\", \"stt.record\"]",
    );
    let engine = voice(
        &home,
        &program,
        "engine-a",
        &["stt.engine.transcribe"],
        "plugs = [\"stt.engine\"]\nmethods = [\"stt.engine.transcribe\"]",
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let before = client.call("l0", "bus.list", json!({})).await;
    assert_eq!(
        before["result"]["names"],
        json!([
            {"name": "stt", "kind": "provides", "package": "voice-in", "ready": false, "methods": []},
            {"name": "stt.engine", "kind": "plugs", "package": "engine-a", "ready": false, "methods": []},
        ]),
        "还没起来的不算可用：{before}"
    );
    started(&mut client, "voice-in", &stt, 2).await;
    started(&mut client, "engine-a", &engine, 1).await;
    let after = client
        .call("l1", "bus.list", json!({"name": "stt.engine"}))
        .await;
    assert_eq!(
        after["result"]["names"],
        json!([{"name": "stt.engine", "kind": "plugs", "package": "engine-a", "ready": true, "methods": ["transcribe"]}]),
        "{after}"
    );
    let stt_ready = client.call("l2", "bus.list", json!({"name": "stt"})).await;
    assert_eq!(
        stt_ready["result"]["names"][0]["ready"], false,
        "导出了 stt.record 却没登记，不算可用：{stt_ready}"
    );
    // 登记上了哪几个照短名列出来（施工 BUS-3补）：接进同一个名字的几家各实现了哪几处，调的一方照它认，不用另外去读清单。
    assert_eq!(
        stt_ready["result"]["names"][0]["methods"],
        json!(["coded", "transcribe"]),
        "只列登记上的、照字母排，没登记的 stt.record 不在：{stt_ready}"
    );

    let said = bus(
        &mut client,
        "c1",
        json!({"name": "stt", "method": "transcribe", "params": {"x": 1}}),
    )
    .await;
    assert_eq!(
        said["result"],
        json!({"answered": "stt.transcribe"}),
        "{said}"
    );
    let plugged = bus(
        &mut client,
        "c2",
        json!({"name": "stt.engine", "method": "transcribe", "package": "engine-a"}),
    )
    .await;
    assert_eq!(
        plugged["result"],
        json!({"answered": "stt.engine.transcribe"}),
        "{plugged}"
    );

    for (id, params, want) in [
        (
            "c3",
            json!({"name": "stt.engine", "method": "transcribe"}),
            "package_required",
        ),
        (
            "c4",
            json!({"name": "stt.engine", "method": "transcribe", "package": "voice-in"}),
            "no_provider",
        ),
        (
            "c5",
            json!({"name": "tts", "method": "play"}),
            "no_provider",
        ),
        (
            "c6",
            json!({"name": "stt", "method": "engine.transcribe"}),
            "bad_params",
        ),
        (
            "c7",
            json!({"name": "stt", "method": "stop"}),
            "not_exported",
        ),
    ] {
        let reply = bus(&mut client, id, params.clone()).await;
        assert_eq!(reason(&reply), want, "{params}：{reply}");
    }
    let coded = bus(&mut client, "c8", json!({"name": "stt", "method": "coded"})).await;
    assert_eq!(reason(&coded), "method_failed", "{coded}");
    assert_eq!(
        coded["error"]["data"]["code"], "bad_audio",
        "码照 error.data.code：{coded}"
    );
    core.stop_extensions().await;
}

/// `bus` 流（施工 BUS-2，B9）：订阅回整份名字；扩展起来、登记了方法，推 `bus.changed`，带整份名字，可用跟着变。
#[tokio::test]
async fn the_bus_stream_pushes_the_whole_list_when_readiness_changes() {
    let home = Home::new();
    let program = Program::new();
    let stt = voice(
        &home,
        &program,
        "voice-in",
        &["stt.transcribe"],
        "provides = [\"stt\"]\nmethods = [\"stt.transcribe\"]",
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let subscribed = client
        .call("s1", "subscribe", json!({"stream": "bus"}))
        .await;
    assert_eq!(
        subscribed["result"]["names"],
        json!([{"name": "stt", "kind": "provides", "package": "voice-in", "ready": false, "methods": []}]),
        "{subscribed}"
    );
    let on = json!({"jsonrpc": "2.0", "id": "e", "method": "package.enable", "params": {"package": "voice-in"}});
    client.line(&on.to_string()).await;
    let ready = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let next = client.next().await.expect("没断开");
            if next["method"] == "bus.changed" && next["params"]["names"][0]["ready"] == true {
                return next;
            }
        }
    })
    .await;
    assert!(
        ready.is_ok(),
        "登记了以后推了可用的那一份：{}",
        std::fs::read_to_string(&stt).unwrap_or_default()
    );
    core.stop_extensions().await;
}

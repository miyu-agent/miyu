//! 会话状态（施工 9-8 补上，`docs/blueprint/view.md`「会话状态」）：订阅视图流的回应带整份 `status`；跑一轮时推 `view.status`，
//! 先 `running`、最后 `idle`，用量跟着变；切权限、改待办这些落了盘的变化也推；没变的不推。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 订阅视图流，交回回应。
async fn subscribe_view(client: &mut Client, session: &str) -> Value {
    client
        .call(
            "v1",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await
}

/// 发一条请求，读到它的回应和 `until` 说停为止：交回之间推来的。
async fn through(
    client: &mut Client,
    id: &str,
    method: &str,
    params: Value,
    until: impl Fn(&Value) -> bool,
) -> Vec<Value> {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    client.line(&request.to_string()).await;
    let mut pushed = Vec::new();
    let (mut replied, mut done) = (false, false);
    while !(replied && done) {
        let next = client.next().await.expect("没断开");
        if next["id"] == json!(id) {
            replied = true;
            continue;
        }
        done |= until(&next);
        pushed.push(next);
    }
    pushed
}

fn statuses(pushed: &[Value]) -> Vec<&Value> {
    pushed
        .iter()
        .filter(|push| push["method"] == "view.status")
        .map(|push| &push["params"]["status"])
        .collect()
}

fn idle(push: &Value) -> bool {
    push["method"] == "view.status" && push["params"]["status"]["state"] == "idle"
}

#[tokio::test]
async fn the_subscription_carries_the_whole_status() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = subscribe_view(&mut client, &session).await;
    let status = &reply["result"]["status"];
    assert_eq!(status["state"], "idle", "{reply}");
    assert!(status["context"].is_object(), "{status}");
    assert!(status["usage"].is_object(), "{status}");
    assert!(status["permission"].is_object(), "{status}");
    assert!(status["workspace"]["cwd"].is_string(), "{status}");
    assert_eq!(status["todos"], json!([]));
    assert_eq!(status["jobs"], json!([]));
    assert_eq!(status["preset"], reply["result"]["preset"]);
}

#[tokio::test]
async fn a_turn_runs_and_comes_back_idle_with_usage() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = subscribe_view(&mut client, &session).await;
    let before = reply["result"]["status"]["usage"]["requests"].clone();
    let pushed = through(
        &mut client,
        "s1",
        "session.send",
        json!({"session": session, "text": "hi"}),
        idle,
    )
    .await;
    let seen = statuses(&pushed);
    assert!(
        seen.iter()
            .any(|status| status["state"] == "running" && status["since"].is_string()),
        "{seen:#?}"
    );
    let last = seen.last().expect("推过状态");
    assert_eq!(last["state"], "idle");
    assert!(last.get("since").is_none(), "{last}");
    assert_ne!(last["usage"]["requests"], before, "用量跟着变：{last}");
    for pair in seen.windows(2) {
        assert_ne!(pair[0], pair[1], "没变的不推");
    }
}

#[tokio::test]
async fn a_permission_change_is_pushed() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = subscribe_view(&mut client, &session).await;
    assert_eq!(
        reply["result"]["status"]["permission"]["level"],
        "workspace"
    );
    let pushed = through(
        &mut client,
        "p1",
        "session.set_permission_level",
        json!({"session": session, "level": "full"}),
        |push| push["method"] == "view.status",
    )
    .await;
    let last = statuses(&pushed).pop().expect("推了状态");
    assert_eq!(last["permission"]["level"], "full", "{last}");
}

#[tokio::test]
async fn the_persona_and_preset_are_the_sessions() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create_as("c1", "~", "engineer").await;
    let reply = subscribe_view(&mut client, &session).await;
    let status = &reply["result"]["status"];
    assert_eq!(status["persona"], "engineer", "{reply}");
    assert_eq!(status["preset"], reply["result"]["preset"]);
    assert_ne!(status["persona"], status["preset"]);
}

/// 一次全做完、清空了待办（施工 9-8 九补）：推的会话状态里 `todos` 是空的，`todos_done` 是清空前那一份（同事件流
/// `todos.changed` 的 `done`）；再写一份没做完的，`todos_done` 没了。
#[tokio::test]
async fn a_list_cleared_by_finishing_keeps_what_was_done() {
    let home = Home::new();
    let done = serde_json::json!([
        {"content": "读代码", "status": "completed"},
        {"content": "写测试", "status": "completed"}
    ]);
    let again = serde_json::json!([{"content": "跑测试", "status": "pending"}]);
    let write = |todos: &Value| json!({ "todos": todos }).to_string();
    let script = Script::new([
        Play::calls(&[("todowrite", &write(&done))]),
        Play::Says("好。"),
        Play::calls(&[("todowrite", &write(&again))]),
        Play::Says("好。"),
    ]);
    let tools =
        miyu_tool::Catalog::new(miyu_basesystem::tools(&default_resources()).expect("读得出"))
            .expect("合写法");
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let session = client.create("c1", "~").await;
    subscribe_view(&mut client, &session).await;
    let pushed = through(
        &mut client,
        "s1",
        "session.send",
        json!({"session": session, "text": "做完"}),
        idle,
    )
    .await;
    let last = *statuses(&pushed).last().expect("推过状态");
    assert_eq!(last["todos"], json!([]), "{last}");
    assert_eq!(last["todos_done"], done, "{last}");
    let pushed = through(
        &mut client,
        "s2",
        "session.send",
        json!({"session": session, "text": "再来"}),
        idle,
    )
    .await;
    let last = *statuses(&pushed).last().expect("推过状态");
    assert_eq!(last["todos"], again, "{last}");
    assert!(last.get("todos_done").is_none(), "{last}");
}

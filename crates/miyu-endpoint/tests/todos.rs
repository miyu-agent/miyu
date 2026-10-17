//! 待办（施工 D-3，`docs/blueprint/protocol.md` 的 `subscribe`、推送的 `todos.changed`）：真核心、真的 `todowrite`。还没写过的，
//! `subscribe` 的回应不带 `todos`；她写了一份，订阅着的头收到 `todos.changed`；之后再订阅的，回应里带着当前的这一份。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 发一条请求，交回回应之前读到的推送和回应。
async fn request(
    client: &mut Client,
    id: &str,
    method: &str,
    params: Value,
) -> (Vec<Value>, Value) {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    client.line(&request.to_string()).await;
    client.until_reply(id).await
}

/// 推送里的 `todos.changed` 的 `todos`，照先后。
fn changed(pushed: &[Value]) -> Vec<Value> {
    pushed
        .iter()
        .filter(|push| push["params"]["event"]["kind"] == json!("todos.changed"))
        .map(|push| push["params"]["event"]["body"]["todos"].clone())
        .collect()
}

#[tokio::test]
async fn heads_get_the_list_from_subscribe_and_from_pushes() {
    let home = Home::new();
    let todos = json!([
        {"content": "读代码", "status": "completed"},
        {"content": "写测试", "status": "in_progress"}
    ]);
    let args = json!({ "todos": todos }).to_string();
    let script = Script::new([Play::calls(&[("todowrite", &args)]), Play::Says("好。")]);
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("读得出"))
        .expect("合写法");
    let core = home.core_with_tools(&script, tools, TOKEN);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let created = client
        .call("create-1", "session.create", json!({ "cwd": cwd }))
        .await;
    let session = created["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("应该造出会话：{created}"))
        .to_string();
    let (_, reply) = request(
        &mut client,
        "sub-1",
        "subscribe",
        json!({"session": session, "stream": "events"}),
    )
    .await;
    assert!(reply["result"].get("todos").is_none(), "还没写过：{reply}");
    let (mut pushed, _) = request(
        &mut client,
        "send-1",
        "session.send",
        json!({"session": session, "text": "开工"}),
    )
    .await;
    home.until_turns(&session, 1).await;
    // 推送走订阅的转发任务，可能落在不相干的回应后面：一直问到收到为止，最多 100 次。
    for k in 0..100 {
        if !changed(&pushed).is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let (more, _) = request(&mut client, &format!("list-{k}"), "session.list", json!({})).await;
        pushed.extend(more);
    }
    assert_eq!(
        changed(&pushed),
        std::slice::from_ref(&todos),
        "写了一份，推一次"
    );
    // 另一个头这时订阅：回应里就有当前的这一份。
    let mut other = Client::connect(core);
    other.hello().await;
    let (_, reply) = request(
        &mut other,
        "sub-2",
        "subscribe",
        json!({"session": session, "stream": "events"}),
    )
    .await;
    assert_eq!(reply["result"]["todos"], todos, "{reply}");
}

/// 她写好一份待办、回合结束了的会话（施工 D-3 再补）：交回核心、客户端、会话编号。
async fn with_a_list(home: &Home, todos: &Value) -> (Client, String) {
    let args = json!({ "todos": todos }).to_string();
    let script = Script::new([Play::calls(&[("todowrite", &args)]), Play::Says("好。")]);
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("读得出"))
        .expect("合写法");
    let core = home.core_with_tools(&script, tools, TOKEN);
    let mut client = Client::connect(core);
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "开工").await;
    home.until_turns(&session, 1).await;
    (client, session)
}

/// `/todo` 的回执。
async fn todo(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(id, "command.run", json!({"session": session, "text": text}))
        .await
}

/// 日志里的 `todos.cleared`：谁、起因。
fn cleared(home: &Home, session: &str) -> Vec<(miyu_kernel::origin::By, String)> {
    home.log(session)
        .iter()
        .filter(|event| matches!(event.body, miyu_kernel::event::Body::TodosCleared(_)))
        .map(|event| {
            let cause = event.cause.as_ref().map(|id| id.as_str().to_string());
            (event.by.clone(), cause.unwrap_or_default())
        })
        .collect()
}

/// `/todo` 看、`/todo clear` 清（施工 D-3 再补，2026-10-17 项目主人定）：看的一项一行、什么都不记；清的记一条 `todos.cleared`，
/// 再看是没有；没有了再清说没有、什么都不记；后面跟别的字的参数不对。
#[tokio::test]
async fn the_todo_command_shows_and_clears_the_list() {
    let home = Home::new();
    let todos = json!([
        {"content": "读代码", "status": "completed"},
        {"content": "写测试", "status": "in_progress"},
        {"content": "提交", "status": "pending"}
    ]);
    let (mut client, session) = with_a_list(&home, &todos).await;

    let reply = todo(&mut client, "k1", &session, "/todo").await;
    assert_eq!(reply["result"]["command"], "todo", "{reply}");
    assert_eq!(
        reply["result"]["said"],
        "读代码 · 已完成；写测试 · 进行中；提交 · 未开始"
    );
    assert!(cleared(&home, &session).is_empty(), "看的什么都不清");

    let reply = todo(&mut client, "k2", &session, "/todo nope").await;
    assert_eq!(reply["error"]["data"]["reason"], "bad_params", "{reply}");

    let reply = todo(&mut client, "k3", &session, "/todo clear").await;
    assert_eq!(reply["result"]["said"], "已清空待办。", "{reply}");
    assert_eq!(
        cleared(&home, &session),
        [(
            miyu_kernel::origin::By::Person(miyu_kernel::origin::Person::new(alice())),
            "k3".to_string()
        )]
    );
    let reply = todo(&mut client, "k4", &session, "/todo").await;
    assert_eq!(reply["result"]["said"], "无待办。");

    let reply = todo(&mut client, "k5", &session, "/todo clear").await;
    assert_eq!(reply["result"]["said"], "无待办。", "{reply}");
    assert_eq!(cleared(&home, &session).len(), 1, "没有的不再记");
}

/// `/clear` 连待办一起清（施工 D-3 再补）。
#[tokio::test]
async fn clearing_the_context_clears_the_list_too() {
    let home = Home::new();
    let todos = json!([{"content": "读代码", "status": "in_progress"}]);
    let (mut client, session) = with_a_list(&home, &todos).await;
    let reply = todo(&mut client, "k1", &session, "/clear").await;
    assert_eq!(reply["result"]["said"], "已清空上下文。", "{reply}");
    let reply = todo(&mut client, "k2", &session, "/todo").await;
    assert_eq!(reply["result"]["said"], "无待办。");
}

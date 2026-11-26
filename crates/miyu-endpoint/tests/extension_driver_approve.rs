//! 扩展驱动在一次 `driver.call` 进行中请核心问人（施工 ACP-8），真核心、真扩展进程走一遍：
//!
//! - 问到：日志里一条 `tool.approval_requested`，`by` 是包，`driver` 写画在第几次请求的第几块，`detail` 照跑命令的样子写；视图里
//!   那一步供应商侧工具带着这一问、调用编号是这一问的（头照它回答）；
//! - 人允许、拒绝：照 `session.answer` 回，扩展收到 `once`、`deny` 和对得上的选项，拒绝的带理由；回合照常说完；
//! - 没人能确认的会话（握手不能输入的连接开的）：不记请求，当场回 `deny`，`auto` 是 `unattended`；
//! - 问着的时候打断：扩展收到 `cancelled`，日志里内核替人记了拒绝；
//! - 不是这一次的 `call` 回 `unknown_call`，还没开始的块回 `bad_params`，这一轮照常说完。

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::hosted::*;
use crate::support::*;
use miyu_http::testkit::Server;

/// 装好照 `how` 答 `driver.call` 的测试扩展、起核心、开一个会话（`client` 能输入）。程序、假服务器跟着留到测试完。
struct Ready {
    home: Home,
    path: std::path::PathBuf,
    client: Client,
    core: std::sync::Arc<miyu_endpoint::Core>,
    session: String,
    _kept: (Program, Server),
}

async fn ready(how: &str) -> Ready {
    let server = Server::start(Vec::new()).await;
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "ag");
    let driver = format!("driver:{how}");
    install_hosted(
        &home,
        &program.name(),
        &steps(&[&step, "hello", &driver, "serve"]),
    );
    let (mut client, core) = connected(&home, &server).await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "titles": false}))
        .await;
    Ready {
        home,
        path,
        client,
        core,
        session,
        _kept: (program, server),
    }
}

/// 记下的核心回的 `driver.approve`，照先后。
fn approvals(path: &std::path::Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter(|line| line.contains(r#""id":"approve-"#) && !line.contains(r#""method""#))
        .map(|line| serde_json::from_str(line).expect("JSON"))
        .collect()
}

/// 等到日志里有一条 `tool.approval_requested`，交回它。
async fn requested(home: &Home, session: &str) -> Value {
    let find = || {
        logged(home, session)
            .into_iter()
            .find(|event| event["kind"] == "tool.approval_requested")
    };
    until("确认请求", || find().is_some()).await;
    find().expect("有")
}

/// 读视图流，读到一条合 `wanted` 的工具那一步为止，交回它。
async fn until_view_step(viewer: &mut Client, wanted: impl Fn(&Value) -> bool) -> Value {
    loop {
        let next = viewer.next().await.expect("没断开");
        let entry = &next["params"]["entry"];
        let pushed = next["method"] == "view.add" || next["method"] == "view.update";
        if pushed && entry["kind"] == "tool" && wanted(entry) {
            return entry.clone();
        }
    }
}

/// 说一句，不等说完。
async fn send(client: &mut Client, session: &str, text: &str) {
    let reply = client
        .call(
            "s1",
            "session.send",
            json!({"session": session, "text": text}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
}

#[tokio::test]
async fn the_person_allowing_lets_the_driver_go_on() {
    let Ready {
        home,
        path,
        mut client,
        core,
        session,
        ..
    } = ready("approve").await;
    let mut viewer = Client::connect(std::sync::Arc::clone(&core));
    viewer.hello().await;
    let reply = viewer
        .call(
            "v1",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    assert!(reply["result"].is_object(), "{reply}");
    send(&mut client, &session, "alpha").await;
    let request = requested(&home, &session).await;
    assert_eq!(request["by"], json!({"kind": "module", "id": "ag"}));
    let body = &request["body"];
    assert_eq!(body["access"], "execute");
    assert_eq!(body["driver"]["index"], 0, "{body}");
    assert!(body["driver"]["seen"].is_u64(), "{body}");
    assert!(body.get("rule").is_none(), "没有规则：{body}");
    assert_eq!(
        body["detail"],
        json!({"driver": "ag/ag", "tool": "List files", "title": "List files", "command": "ls -la"})
    );
    let call = body["call_id"].clone();
    assert_eq!(
        call.as_str(),
        Some(format!("call_{}_1", request["seq"]).as_str()),
        "内核另给的编号"
    );
    assert!(approvals(&path).is_empty(), "还没答");

    // 视图流：那一步供应商侧工具带着这一问，调用编号是这一问的（头照它回答）。
    let step = until_view_step(&mut viewer, |step| step["approval"].is_object()).await;
    assert_eq!(step["hosted"], true, "{step}");
    assert_eq!(step["call"], call, "{step}");
    assert_eq!(step["approval"]["access"], "execute", "{step}");
    assert!(step["approval"].get("decision").is_none(), "{step}");

    let reply = client
        .call(
            "a1",
            "session.answer",
            json!({"session": session, "call": call, "decision": "once"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    let answered = approvals(&path);
    assert_eq!(
        answered[0]["result"],
        json!({"decision": "once", "option": "yes"}),
        "{answered:?}"
    );
    let replies = replies(&home, &session);
    assert_eq!(replies[0][1]["text"], "echo: alpha", "{replies:?}");
    let decided = logged(&home, &session)
        .into_iter()
        .find(|event| event["kind"] == "tool.approval_decided")
        .expect("记了决定");
    assert_eq!(decided["body"]["decision"], "once");
    // 读回日志的视图：请求排在回复前面，那一步开了条目再写上，定了的是 `once`。
    let page = client
        .call("p", "view.page", json!({"session": session, "view": true}))
        .await;
    let step = page["result"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{page}"))
        .iter()
        .find(|entry| entry["kind"] == "tool")
        .cloned()
        .unwrap_or_else(|| panic!("{page}"));
    assert_eq!(step["hosted"], true, "{step}");
    assert_eq!(step["approval"]["decision"], "once", "{step}");
    assert_ne!(step["call"], call, "落了盘换成那件工具自己的编号：{step}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn the_person_denying_hands_the_reason_to_the_driver() {
    let Ready {
        home,
        path,
        mut client,
        core,
        session,
        ..
    } = ready("approve").await;
    send(&mut client, &session, "alpha").await;
    let call = requested(&home, &session).await["body"]["call_id"].clone();
    let reply = client
        .call(
            "a1",
            "session.answer",
            json!({"session": session, "call": call, "decision": "session"}),
        )
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "no_rule", "{reply}");
    let reply = client
        .call(
            "a2",
            "session.answer",
            json!({"session": session, "call": call, "decision": "deny", "reason": "不要"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    assert_eq!(
        approvals(&path)[0]["result"],
        json!({"decision": "deny", "option": "no", "reason": "不要"})
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn where_no_one_can_approve_the_driver_is_refused_on_the_spot() {
    let Ready {
        home, path, core, ..
    } = ready("approve").await;
    let mut headless = Client::connect(std::sync::Arc::clone(&core));
    headless.hello_without_input().await;
    let session = headless
        .create_with("c2", json!({"cwd": "~", "titles": false}))
        .await;
    send(&mut headless, &session, "alpha").await;
    home.until_turns(&session, 1).await;
    assert_eq!(
        approvals(&path)[0]["result"],
        json!({"decision": "deny", "option": "no", "auto": "unattended"})
    );
    assert!(
        !logged(&home, &session)
            .iter()
            .any(|event| event["kind"] == "tool.approval_requested"),
        "不记请求"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn interrupting_while_asking_tells_the_driver_it_is_cancelled() {
    let Ready {
        home,
        path,
        mut client,
        core,
        session,
        ..
    } = ready("approve").await;
    send(&mut client, &session, "alpha").await;
    let call = requested(&home, &session).await["body"]["call_id"].clone();
    let reply = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "keep"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    until("扩展收到回应", || !approvals(&path).is_empty()).await;
    assert_eq!(
        approvals(&path)[0]["result"],
        json!({"decision": "cancelled"})
    );
    let decided = logged(&home, &session)
        .into_iter()
        .find(|event| event["kind"] == "tool.approval_decided")
        .expect("内核替人记了拒绝");
    assert_eq!(decided["by"], json!({"kind": "kernel"}));
    assert_eq!(decided["body"]["call_id"], call);
    assert_eq!(decided["body"]["decision"], "deny");
    let reply = client
        .call(
            "a1",
            "session.answer",
            json!({"session": session, "call": call, "decision": "once"}),
        )
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "not_asking", "{reply}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn asking_about_another_call_or_a_block_not_started_is_refused() {
    let Ready {
        home,
        path,
        mut client,
        core,
        session,
        ..
    } = ready("approve-other").await;
    send(&mut client, &session, "alpha").await;
    home.until_turns(&session, 1).await;
    let answered = approvals(&path);
    assert_eq!(answered.len(), 2, "{answered:?}");
    assert_eq!(
        answered[0]["error"]["data"]["reason"], "unknown_call",
        "{answered:?}"
    );
    assert_eq!(
        answered[1]["error"]["data"]["reason"], "bad_params",
        "{answered:?}"
    );
    assert!(
        answered[1]["error"]["data"]["why"]
            .as_str()
            .is_some_and(|why| why.contains("index")),
        "{answered:?}"
    );
    assert_eq!(replies(&home, &session)[0][1]["text"], "echo: alpha");
    core.stop_extensions().await;
}

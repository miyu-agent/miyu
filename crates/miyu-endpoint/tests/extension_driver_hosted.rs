//! 扩展驱动交上来的供应商侧工具，真核心、真扩展进程走一遍（设计 71 第三节第 1 到 7 条）：从扩展进程到日志、视图、下一轮的请求。
//!
//! - 一轮 `driver:hosted`：`message.assistant` 里有一块 `hosted_tool`（名字、参数、结果对，私有数据记成 `<包>/<驱动>` 的家族），
//!   回合照常说完，一件工具都没跑；
//! - `view.page` 里是一步工具：`hosted` 真、`via` 是这家供应商的显示名、成了、`output` 是结果；
//! - 同一个会话再说一轮：交给扩展的 `driver.call` 的 `request` 里上一轮那块原样在（自己家、带私有数据）；
//! - 换到一个内置驱动的模型再说一轮：发出去的请求里它是那段 `<hosted-tool name="search_web">` 的字。
//!
//! - 装之前那一屏（`package.info` 的 `contains`，同看一眼）带 `hosted`：驱动名到它会让供应商自己跑的工具名。
//!
//! 没声明的名字照收、只说一次的 `WARN` 在 `extension_driver_hosted_log.rs`（装日志订阅者的单独一个程序）。

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::hosted::*;
use crate::support::providers::said;
use crate::support::*;
use miyu_http::testkit::Server;

#[tokio::test]
async fn a_hosted_tool_goes_from_the_extension_to_the_log_the_view_and_the_next_requests() {
    let server = Server::start(vec![said("Done.")]).await;
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "ag");
    install_hosted(
        &home,
        &program.name(),
        &steps(&[&step, "hello", "driver:hosted", "serve"]),
    );
    let (mut client, core) = connected(&home, &server).await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "titles": false}))
        .await;
    say(&mut client, &home, &session, "alpha", 1).await;

    // 日志：一块供应商侧工具、一块正文；回合照常说完，一件工具都没跑。
    let blocks = &replies(&home, &session)[0];
    let hosted = &blocks[0];
    assert_eq!(hosted["type"], "hosted_tool", "{blocks}");
    assert_eq!(hosted["name"], "search_web");
    assert_eq!(hosted["args"], r#"{"query":"rust"}"#, "两段参数接起来");
    assert_eq!(
        hosted["outcome"],
        json!({"text": "1. Rust https://www.rust-lang.org"}),
        "没失败、没截的不写"
    );
    assert_eq!(
        hosted["private"],
        json!({"driver": "ag/ag", "data": {"step": 7}}),
        "私有数据记成扩展驱动的家族"
    );
    assert_eq!(blocks[1]["text"], "echo: alpha", "{blocks}");
    let log = logged(&home, &session);
    assert!(
        !log.iter()
            .any(|event| event["kind"] == "tool.called" || event["kind"] == "tool.result"),
        "供应商侧工具核心不跑"
    );
    let ended: Vec<&Value> = log
        .iter()
        .filter(|event| event["kind"] == "turn.ended")
        .map(|event| &event["body"]["reason"])
        .collect();
    assert_eq!(ended, ["completed"], "回合照常说完：{log:#?}");

    // 视图：一步工具，标着是 Gravity 跑的、成了、结果是供应商交的。
    let page = client
        .call("p", "view.page", json!({"session": session, "view": true}))
        .await;
    let tools: Vec<&Value> = page["result"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{page}"))
        .iter()
        .filter(|entry| entry["kind"] == "tool")
        .collect();
    assert_eq!(tools.len(), 1, "{page}");
    let step = tools[0];
    assert_eq!(step["hosted"], true, "{step}");
    assert_eq!(step["name"], "search_web", "{step}");
    assert_eq!(step["title"]["via"], "Gravity", "{step}");
    assert_eq!(step["state"], "ok", "{step}");
    assert_eq!(
        step["output"], "1. Rust https://www.rust-lang.org",
        "{step}"
    );

    // 再说一轮：上一轮那块原样交回给扩展（自己家、带私有数据）。
    say(&mut client, &home, &session, "beta", 2).await;
    let recorded = read(&path);
    let calls: Vec<Value> = recorded
        .lines()
        .filter(|line| line.contains(r#""method":"driver.call""#))
        .map(|line| serde_json::from_str(line).expect("JSON"))
        .collect();
    assert_eq!(calls.len(), 2, "{recorded}");
    let handed: Vec<&Value> = calls[1]["params"]["request"]["messages"]
        .as_array()
        .expect("消息")
        .iter()
        .flat_map(|message| message["blocks"].as_array().into_iter().flatten())
        .filter(|block| block["type"] == "hosted_tool")
        .collect();
    assert_eq!(handed, [hosted], "原样带回：{recorded}");

    // 换到内置驱动的模型再说一轮：它是一段字。
    let reply = client
        .call(
            "m",
            "session.configure",
            json!({"session": session, "model": "o/m"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    say(&mut client, &home, &session, "gamma", 3).await;
    let received = server.received();
    assert_eq!(received.len(), 1, "只有这一轮走内置驱动");
    let body: Value = serde_json::from_slice(&received[0].body).expect("请求体是 JSON");
    let retold = body.to_string();
    assert!(
        retold.contains(r#"<hosted-tool name=\"search_web\">"#),
        "换成那段字：{retold}"
    );
    assert!(
        retold.contains("1. Rust https://www.rust-lang.org"),
        "{retold}"
    );
    assert!(
        !retold.contains("\"step\":7"),
        "别家的私有数据不交：{retold}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn the_install_screen_lists_what_the_provider_runs_on_its_own() {
    let server = Server::start(Vec::new()).await;
    let home = Home::new();
    let program = Program::new();
    install_hosted(&home, &program.name(), &steps(&["hello", "serve"]));
    let (mut client, core) = connected(&home, &server).await;
    let reply = client
        .call("i", "package.info", json!({"package": "ag"}))
        .await;
    let contains = &reply["result"]["contains"];
    assert_eq!(contains["drivers"], json!(["ag"]), "{reply}");
    assert_eq!(
        contains["hosted"],
        json!({"ag": ["search_web"]}),
        "驱动名到工具名：{reply}"
    );
    core.stop_extensions().await;
}

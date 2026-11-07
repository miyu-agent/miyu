//! 扩展提供的模型驱动，真核心、真扩展进程走一遍（施工 MD-2，设计 66 第五、八、九节）：人的连接调 `model.call` 点名用扩展驱动的那一家。
//!
//! - 回答是扩展经 `driver.delta` 交的字；交过去的 key 只有这一家的，别家的 key 不出现在任何一次 `driver.call` 里；
//! - 两个调用同时在路上：扩展交错着交，核心照 `call` 分开，各拿到自己的；
//! - 交了一半扩展崩了：这一次出错（`the driver went away`），不卡住；
//! - 包没批 `model.driver` 的：扩展不拉起，这一家当用不了（原因写明），不发。

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::providers::{data, profiles, routed_serving_with};
use crate::support::*;
use miyu_http::testkit::Server;

/// 装包 `ag`：扩展照 `steps` 跑，声明驱动 `ag`（`key = "optional"`）；`approved` 的批了 `model.driver`。
fn install(home: &Home, program: &str, steps: &[String], approved: bool) {
    let args: Vec<String> = steps.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        "opt/ag/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Ag\" }}\n\n[command]\nname = \"ag\"\nprogram = \"{program}\"\nabout = {{ en = \"A\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"ag\"\n",
            args.join(", ")
        ),
    );
    if approved {
        approve(home, "ag", &["model.driver"]);
    }
}

/// 两家：`g` 走扩展驱动 `ag/ag`、key 照 `G_KEY`；`o` 走内置驱动、指着假服务器，key 照 `O_KEY`。`models.chat` 是 `g/m`。
fn config(server: &Server) -> String {
    format!(
        "[providers.g]\ndriver = \"ag/ag\"\nkey = {{ env = \"G_KEY\" }}\n\n[providers.o]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"O_KEY\" }}\n\n[models]\nchat = \"g/m\"\n",
        server.base_url
    )
}

/// 起核心、连上、等扩展握了手（`running` 的才等）。
async fn connected(
    home: &Home,
    server: &Server,
    running: bool,
) -> (Client, std::sync::Arc<miyu_endpoint::Core>) {
    home.write("etc/config.toml", &config(server));
    let core = routed_serving_with(
        home,
        &[("G_KEY", "sk-g"), ("O_KEY", "sk-other-secret")],
        data(profiles(json!({}))),
    );
    core.start_drivers();
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    if running {
        until_state(&mut client, "ag", |one| one["state"] == "running").await;
    }
    (client, core)
}

fn asked(text: &str) -> Value {
    json!({"purpose": "platform", "model": "g/m", "messages": [{"role": "user", "text": text}]})
}

#[tokio::test]
async fn the_answer_comes_from_the_extension_with_only_its_key() {
    let server = Server::start(Vec::new()).await;
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "ag");
    install(
        &home,
        &program.name(),
        &steps(&[&step, "hello", "driver:echo", "serve"]),
        true,
    );
    let (mut client, core) = connected(&home, &server, true).await;
    let reply = client.call("c1", "model.call", asked("alpha")).await;
    assert_eq!(reply["result"]["text"], "echo: alpha", "{reply}");
    assert_eq!(reply["result"]["provider"], "g", "{reply}");
    assert_eq!(reply["result"]["usage"]["uncached"], 5, "{reply}");
    let recorded = read(&path);
    let calls: Vec<&str> = recorded
        .lines()
        .filter(|line| line.contains(r#""method":"driver.call""#))
        .collect();
    assert_eq!(calls.len(), 1, "{recorded}");
    let call: Value = serde_json::from_str(calls[0]).expect("JSON");
    assert_eq!(call["params"]["key"], "sk-g");
    assert_eq!(call["params"]["driver"], "ag/ag");
    assert!(call["params"].get("base_url").is_none(), "没写地址的不交");
    assert!(
        !recorded.contains("sk-other-secret"),
        "别家的 key 一次都不交"
    );
    assert!(server.received().is_empty(), "没退回内置驱动发");
    core.stop_extensions().await;
}

#[tokio::test]
async fn two_calls_at_once_each_get_their_own_answer() {
    let server = Server::start(Vec::new()).await;
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        &program.name(),
        &steps(&["hello", "driver:pair", "serve"]),
        true,
    );
    let (mut first, core) = connected(&home, &server, true).await;
    let mut second = Client::connect(std::sync::Arc::clone(&core));
    second.hello().await;
    let (one, two) = tokio::join!(
        first.call("c1", "model.call", asked("alpha")),
        second.call("c2", "model.call", asked("beta")),
    );
    assert_eq!(one["result"]["text"], "echo: alpha", "{one}");
    assert_eq!(two["result"]["text"], "echo: beta", "{two}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_crash_halfway_ends_the_call_with_an_error() {
    let server = Server::start(Vec::new()).await;
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        &program.name(),
        &steps(&["hello", "driver:half", "serve"]),
        true,
    );
    let (mut client, core) = connected(&home, &server, true).await;
    let reply = client.call("c1", "model.call", asked("alpha")).await;
    assert!(reply["error"].is_object(), "{reply}");
    assert!(
        reply.to_string().contains("the driver went away"),
        "{reply}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn an_unapproved_package_is_not_used() {
    let server = Server::start(Vec::new()).await;
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        &program.name(),
        &steps(&["hello", "driver:echo", "serve"]),
        false,
    );
    let (mut client, core) = connected(&home, &server, false).await;
    let reply = client.call("c1", "model.call", asked("alpha")).await;
    assert!(
        reply
            .to_string()
            .contains("is not approved to provide model drivers"),
        "{reply}"
    );
    assert!(server.received().is_empty());
    core.stop_extensions().await;
}

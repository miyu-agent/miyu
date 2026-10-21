//! 吃不下空外壳的那一家（施工 L-2，`docs/designs/25-工具加载.md` W9）：配置里手写标了 `compat.strict_tools` 的，发到线上的
//! tools 里要先加载的那一件是完整格式；没标的照旧是外壳；别的工具、会话自己的请求不受影响。

use std::sync::Arc;
use std::time::Duration;

use miyu_config::secret::Reference;
use miyu_http::testkit::Server;
use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Spec, Tool};
use serde_json::Value;

use crate::support::Home;
use crate::support::routing::{configs, hellos, routes, turn};

const FULL: &str =
    r#"{"type":"object","properties":{"word":{"type":"string"}},"required":["word"]}"#;

/// 要先加载的 `lookup` 和全量的 `echo`。
fn catalog() -> Catalog {
    let lookup = Fake::from_spec(
        Spec {
            name: "lookup".to_string(),
            description: "Look a word up.".to_string(),
            parameters: serde_json::from_str::<RawJson>(FULL).expect("是 JSON"),
            access: Access::Read,
            need_load: true,
            clicall: false,
            result: None,
        },
        Act::Echo,
    );
    let echo = Fake::new("echo", Access::Read, Act::Echo);
    Catalog::new([lookup, echo].into_iter().map(|tool| tool as Arc<dyn Tool>)).expect("都合写法")
}

/// 照配置的尾巴 `compat` 说一轮，交回发到假服务器的 tools：名字 → 参数格式。
async fn sent_tools(compat: &str) -> Vec<(String, Value)> {
    let mut home = Home::new();
    let server = Server::start(hellos(1)).await;
    let source = format!(
        "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"TEST_KEY\" }}\n{compat}\n[models]\nchat = \"relay/m-1\"\n",
        server.base_url
    );
    home.configs = configs(
        &source,
        &[(Reference::Env("TEST_KEY".to_string()), "sk-test")],
    );
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = home.create_with(&routes, &catalog()).await;
    turn(&handle, "cmd-1").await;
    let received = server.received();
    assert_eq!(received.len(), 1);
    let body: Value = serde_json::from_slice(&received[0].body).expect("是 JSON");
    body["tools"]
        .as_array()
        .expect("有工具面")
        .iter()
        .map(|tool| {
            let function = &tool["function"];
            let name = function["name"].as_str().expect("有名字").to_string();
            (name, function["parameters"].clone())
        })
        .collect()
}

fn parameters<'a>(tools: &'a [(String, Value)], name: &str) -> &'a Value {
    &tools
        .iter()
        .find(|(tool, _)| tool == name)
        .unwrap_or_else(|| panic!("tools 里有 {name}：{tools:?}"))
        .1
}

#[tokio::test]
async fn a_hand_written_strict_provider_gets_the_full_parameters() {
    let tools = sent_tools("\n[providers.relay.compat]\nstrict_tools = true\n").await;
    let names: Vec<&str> = tools.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["echo", "load_tools", "lookup"], "load_tools 照样在");
    let full: Value = serde_json::from_str(FULL).expect("是 JSON");
    assert_eq!(parameters(&tools, "lookup"), &full);
}

#[tokio::test]
async fn an_unmarked_provider_gets_the_shell() {
    let tools = sent_tools("").await;
    assert_eq!(
        parameters(&tools, "lookup"),
        &serde_json::json!({"type": "object"})
    );
}

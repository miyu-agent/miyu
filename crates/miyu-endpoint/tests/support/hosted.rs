//! 扩展驱动交供应商侧工具的两个测试程序共用的（`extension_driver_hosted.rs`、`extension_driver_hosted_log.rs`）：装测试扩展、
//! 起核心、说一句、读日志里的回复。

use serde_json::{Value, json};

use super::extensions::*;
use super::providers::{data, profiles, routed_serving_with};
use super::*;
use miyu_http::testkit::Server;

/// 装包 `ag`：扩展照 `steps` 跑，声明驱动 `ag`，会让供应商自己跑 `search_web`，批了 `model.driver`。
pub fn install_hosted(home: &Home, program: &str, steps: &[String]) {
    let args: Vec<String> = steps.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        "opt/ag/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Ag\" }}\n\n[command]\nname = \"ag\"\nprogram = \"{program}\"\nabout = {{ en = \"A\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"ag\"\nhosted = [\"search_web\"]\n",
            args.join(", ")
        ),
    );
    approve(home, "ag", &["model.driver"]);
}

/// 两家：`g` 叫 Gravity、走扩展驱动 `ag/ag`；`o` 走内置驱动、指着假服务器。`models.chat` 是 `g/m`。起核心、连上、等扩展握了手。
pub async fn connected(
    home: &Home,
    server: &Server,
) -> (Client, std::sync::Arc<miyu_endpoint::Core>) {
    home.write(
        "etc/config.toml",
        &format!(
            "[providers.g]\nname = \"Gravity\"\ndriver = \"ag/ag\"\n\n[providers.o]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"O_KEY\" }}\n\n[models]\nchat = \"g/m\"\n",
            server.base_url
        ),
    );
    let core = routed_serving_with(home, &[("O_KEY", "sk-o")], data(profiles(json!({}))));
    core.start_drivers();
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    until_state(&mut client, "ag", |one| one["state"] == "running").await;
    (client, core)
}

/// 说一句 `text`，等会话说完第 `turns` 轮。
pub async fn say(client: &mut Client, home: &Home, session: &str, text: &str, turns: usize) {
    let params = json!({"session": session, "text": text});
    let reply = client
        .call(&format!("s{turns}"), "session.send", params)
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(session, turns).await;
}

/// 日志里每一条回复的块，照先后。
pub fn replies(home: &Home, session: &str) -> Vec<Value> {
    logged(home, session)
        .into_iter()
        .filter(|event| event["kind"] == "message.assistant")
        .map(|event| event["body"]["blocks"].clone())
        .collect()
}

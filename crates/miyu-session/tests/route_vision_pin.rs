//! 会话钉的看图模型（施工 VM-1，`docs/blueprint/models.md`「怎么走」第十三条第 4 条）：钉了的替看图发给它，`models.vision`
//! 照旧配着也不问；去掉的回到 `models.vision`；重启以后照日志算回来；钉的解析不出的照「`models.vision` 解析不出」那条路：
//! 这张图写占位、主请求照发，不退回配置的那一个。
//!
//! 三台假服务器：`a` 是会话的模型 `a/m`（看不了图），`b` 是配置的看图模型 `b/v`，`c` 是钉的 `c/w`。

use std::time::Duration;

use crate::support::routing::{configs, hellos, routes};
use crate::support::{Home, ask, stop, until_turn_ends, watch};
use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Image};
use miyu_kernel::id::{ContentHash, MediaType};
use miyu_kernel::session::Command;
use miyu_session::{Handle, Routes};
use miyu_store::blob::Blobs;

fn config(chat: &Server, configured: &Server, pinned: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nwindow = 100000\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.v]\ninputs = [\"text\", \"image\"]\n\n\
         [providers.c]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.c.models.w]\ninputs = [\"text\", \"image\"]\n\n\
         [models]\nchat = \"a/m\"\nvision = \"b/v\"\n",
        chat.base_url, configured.base_url, pinned.base_url
    )
}

/// 存进 blob 的一张「图」：每次字节不同，各是一张新图。
fn stored(home: &Home, bytes: &[u8]) -> ContentHash {
    Blobs::new(home.root.blobs()).put(bytes).expect("存得进去")
}

/// 只发一张图，等这一轮说完。
async fn show(handle: &Handle, command: &str, blob: &ContentHash) {
    let mut pushes = watch(handle).await;
    let send = Command::Send {
        blocks: vec![Block::Image(Image {
            blob: blob.clone(),
            name: None,
            media_type: MediaType::parse("image/png").expect("媒体类型合写法"),
            width: 1,
            height: 1,
            path: None,
        })],
        urgent: false,
        venue: None,
    };
    ask(handle, command, send).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 钉成 `vision`（`None` 是去掉）。
async fn pin(handle: &Handle, command: &str, vision: Option<&str>) {
    let set = Command::SetVision {
        vision: vision.map(str::to_string),
    };
    ask(handle, command, set).await.expect("会话在跑");
}

/// 一台服务器收到的请求里的 `model`，照先后。
fn models(server: &Server) -> Vec<String> {
    server
        .received()
        .iter()
        .map(|received| {
            let body: serde_json::Value =
                serde_json::from_slice(&received.body).expect("请求是 JSON");
            body["model"].as_str().unwrap_or_default().to_string()
        })
        .collect()
}

fn plain() -> Routes {
    routes(serde_json::json!({}), Duration::from_secs(60))
}

#[tokio::test]
async fn the_pin_wins_over_models_vision_and_survives_a_restart() {
    let (chat, configured, pinned) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(2)).await,
        Server::start(hellos(2)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&chat, &configured, &pinned), &[]);
    let routes = plain();
    let handle = home.create(&routes).await;
    show(&handle, "cmd-1", &stored(&home, b"one")).await;
    assert_eq!(models(&configured), ["v"], "没钉的照 models.vision");
    pin(&handle, "cmd-2", Some("c/w")).await;
    show(&handle, "cmd-3", &stored(&home, b"two")).await;
    assert_eq!(models(&pinned), ["w"], "钉了的发给它");
    assert_eq!(configured.received().len(), 1, "配置的不问");
    stop(&handle).await;
    let handle = home.load(handle.id(), &routes).await;
    show(&handle, "cmd-4", &stored(&home, b"three")).await;
    assert_eq!(models(&pinned), ["w", "w"], "重启以后照日志算的");
    pin(&handle, "cmd-5", None).await;
    show(&handle, "cmd-6", &stored(&home, b"four")).await;
    assert_eq!(models(&configured), ["v", "v"], "去掉了回到配置的");
    assert_eq!(pinned.received().len(), 2);
}

#[tokio::test]
async fn a_pin_that_no_longer_resolves_leaves_the_placeholder() {
    let (chat, configured, pinned) = (
        Server::start(hellos(4)).await,
        Server::start(hellos(1)).await,
        Server::start(Vec::new()).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&chat, &configured, &pinned), &[]);
    let routes = plain();
    let handle = home.create(&routes).await;
    // 协议那一头钉的时候查过；这里是钉了以后池被删了的那种。
    pin(&handle, "cmd-1", Some("@gone")).await;
    show(&handle, "cmd-2", &stored(&home, b"one")).await;
    assert!(configured.received().is_empty(), "不退回 models.vision");
    let main = String::from_utf8_lossy(&chat.received()[0].body).into_owned();
    assert!(
        main.contains("An image was attached here, but this model cannot view images."),
        "主请求照发，图写占位：{main}"
    );
}

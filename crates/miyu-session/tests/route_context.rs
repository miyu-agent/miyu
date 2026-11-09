//! 会话的上下文上限（施工 CTX-1，设计 68 第二节）：路由交的限额，窗口取模型的和上限里小的，模型没报窗口的照上限；设了当场
//! 生效、不等下一轮；换模型照用；去掉回到模型的；重启以后照日志读回来。
//!
//! 一台假服务器、没有目录：`a/m` 的窗口 1000000，`b/n` 的 64000，`b/y` 没写窗口。

use std::time::Duration;

use crate::support::routing::{configs, hellos, routes};
use crate::support::{Home, Lines, Opening, ask, say, stop, until_turn_ends, watch};
use miyu_http::testkit::Server;
use miyu_kernel::session::Command;
use miyu_session::{Handle, Models};
use miyu_tool::Catalog;

fn config(server: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{0}\"\n\n[providers.a.models.m]\nwindow = 1000000\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{0}\"\n\n[providers.b.models.n]\nwindow = 64000\n\n\
         [models]\nchat = \"a/m\"\n",
        server.base_url
    )
}

async fn create(home: &Home, models: &dyn Models, model: &str) -> Handle {
    let lines = Lines {
        model: Some(model.to_string()),
        ..Lines::default()
    };
    home.create_full(models, &Catalog::default(), Opening::default(), lines)
        .await
}

/// 说 `words`、等这一轮说完。
async fn turn(handle: &Handle, command: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, command, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

async fn send(handle: &Handle, command: &str, sent: Command) {
    ask(handle, command, sent).await.expect("会话在跑");
}

fn cap(tokens: Option<u64>) -> Command {
    Command::SetContext { tokens }
}

fn switch(model: &str) -> Command {
    Command::Configure {
        model: model.to_string(),
    }
}

/// 给头看的窗口、上限。
fn shown(handle: &Handle) -> (Option<u64>, Option<u64>) {
    let limits = handle.limits();
    (limits.window, limits.context)
}

#[tokio::test]
async fn the_cap_follows_the_session_across_models_and_restarts() {
    let server = Server::start(hellos(24)).await;
    let mut home = Home::new();
    home.configs = configs(&config(&server), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let a = create(&home, &routes, "a/m").await;
    assert_eq!(shown(&a), (Some(1_000_000), None));
    send(&a, "a-1", cap(Some(128_000))).await;
    assert_eq!(
        shown(&a),
        (Some(128_000), Some(128_000)),
        "当场生效，不等下一轮"
    );
    turn(&a, "a-2", "alpha").await;
    assert_eq!(
        shown(&a),
        (Some(128_000), Some(128_000)),
        "回合开始重算照套"
    );
    send(&a, "a-3", switch("b/n")).await;
    turn(&a, "a-4", "bravo").await;
    assert_eq!(
        shown(&a),
        (Some(64_000), Some(128_000)),
        "模型的小：照模型的"
    );
    send(&a, "a-5", switch("b/y")).await;
    turn(&a, "a-6", "charlie").await;
    assert_eq!(
        shown(&a),
        (Some(128_000), Some(128_000)),
        "模型没报窗口：照上限"
    );
    send(&a, "a-7", switch("a/m")).await;
    turn(&a, "a-8", "delta").await;
    assert_eq!(shown(&a), (Some(128_000), Some(128_000)), "换回来照套");
    stop(&a).await;
    let a = home.load(a.id(), &routes).await;
    assert_eq!(
        shown(&a),
        (Some(128_000), Some(128_000)),
        "载入照日志算的造路由"
    );
    send(&a, "a-9", cap(None)).await;
    assert_eq!(shown(&a), (Some(1_000_000), None), "去掉回到模型的");
}

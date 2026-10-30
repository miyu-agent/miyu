//! 会话记的思考强度（施工 SM-1，设计 51，`docs/blueprint/models.md`「怎么走」第十一条）：一个会话设了只管它自己，另一个照
//! 配置；设了的改配置也照旧，没设的跟着配置；`null` 回到跟着配置；换到没有那一档的模型退回配置、`from` 照实写，换回来又
//! 用上；重启以后照日志读回来；换到池上不用会话那一档，成员照配置（SM-1 补）。
//!
//! 一台假服务器，档案是空的、没有目录：档位全照手写的 `reasoning`。两个会话说的话不一样，照话认请求是谁发的。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch as channel;

use crate::support::routing::{configs, hellos, items, routes};
use crate::support::{Home, Lines, Opening, ask, say, stop, until_turn_ends, watch};
use miyu_config::Layer;
use miyu_config::merge::{Layers, merge};
use miyu_config::parse::parse;
use miyu_http::testkit::Server;
use miyu_kernel::event::{EffortInUse, EffortSource};
use miyu_kernel::session::Command;
use miyu_session::{ConfigSource, Handle, Models, fixed_with};
use miyu_tool::Catalog;

/// `a/m` 有 `off`、`low`、`high`，默认 `a_default`；`b/n` 有 `high`、`max`，没有默认。`models.chat` 是 `a/m`。
fn config(first: &Server, a_default: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{0}\"\n\n[providers.a.models.m]\nreasoning = [\"off\", \"low\", \"high\"]\neffort = \"{a_default}\"\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{0}\"\n\n[providers.b.models.n]\nreasoning = [\"high\", \"max\"]\n\n\
         [models]\nchat = \"a/m\"\n",
        first.base_url
    )
}

/// 一份系统配置当成不变的配置源。
fn one_layer(system: &str) -> Arc<dyn ConfigSource> {
    let item_list = items();
    let system = parse(&item_list, Layer::System, system).expect("写法对");
    let layers = Layers {
        system: Some(&system),
        ..Layers::default()
    };
    Arc::clone(&*fixed_with(merge(&item_list, &layers, &|_| None), Vec::new()).borrow())
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

fn effort(level: Option<&str>) -> Command {
    Command::SetEffort {
        level: level.map(str::to_string),
    }
}

/// 带着 `words` 的请求里最后一份的 `reasoning_effort`；没带的是 `-`。
fn last_sent(server: &Server, words: &str) -> String {
    let bodies: Vec<serde_json::Value> = server
        .received()
        .iter()
        .map(|received| serde_json::from_slice(&received.body).expect("请求是 JSON"))
        .filter(|body: &serde_json::Value| body.to_string().contains(words))
        .collect();
    let body = bodies.last().unwrap_or_else(|| panic!("{words} 发过"));
    body.get("reasoning_effort")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
        .to_string()
}

fn used(level: &str, from: EffortSource) -> Option<EffortInUse> {
    Some(EffortInUse {
        level: level.to_string(),
        from,
    })
}

/// A 设了 `high`、B 不动：A 照会话的，B 照配置的；配置改成 `off` 以后 A 照旧、B 跟着；A 写 `null` 回到跟着配置。
#[tokio::test]
async fn one_session_s_level_leaves_the_other_and_the_config_alone() {
    let first = Server::start(hellos(24)).await;
    let system = config(&first, "low");
    let (switching, receiving) = channel::channel(one_layer(&system));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let (a, b) = (
        create(&home, &routes, "a/m").await,
        create(&home, &routes, "a/m").await,
    );
    send(&a, "a-1", effort(Some("high"))).await;
    assert_eq!(
        a.next().effort,
        used("low", EffortSource::System),
        "下一个回合起生效"
    );
    turn(&a, "a-2", "alpha").await;
    turn(&b, "b-1", "bravo").await;
    assert_eq!(last_sent(&first, "alpha"), "high");
    assert_eq!(last_sent(&first, "bravo"), "low", "B 照配置");
    assert_eq!(a.next().effort, used("high", EffortSource::Session));
    assert_eq!(b.next().effort, used("low", EffortSource::System));
    // 改配置：A 照旧，B 跟着。
    switching.send_replace(one_layer(&config(&first, "off")));
    turn(&a, "a-3", "alpha").await;
    turn(&b, "b-2", "bravo").await;
    assert_eq!(last_sent(&first, "alpha"), "high", "A 照会话的");
    assert_eq!(
        last_sent(&first, "bravo"),
        "none",
        "B 跟着配置（off 照没有开关发 none）"
    );
    assert_eq!(b.next().effort, used("off", EffortSource::System));
    // `null`：回到跟着配置。
    send(&a, "a-4", effort(None)).await;
    turn(&a, "a-5", "alpha").await;
    assert_eq!(last_sent(&first, "alpha"), "none");
    assert_eq!(a.next().effort, used("off", EffortSource::System));
}

/// 换到没有那一档的模型：退回配置（`b/n` 没有默认，什么都不带），`from` 照实写；换模型不清会话那一档，换回来又用上；
/// 重启以后照日志读回来。
#[tokio::test]
async fn a_model_without_the_level_falls_back_and_the_level_survives_a_restart() {
    let first = Server::start(hellos(24)).await;
    let mut home = Home::new();
    home.configs = configs(&config(&first, "low"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let a = create(&home, &routes, "a/m").await;
    send(&a, "a-1", effort(Some("off"))).await;
    turn(&a, "a-2", "alpha").await;
    assert_eq!(last_sent(&first, "alpha"), "none");
    assert_eq!(a.next().effort, used("off", EffortSource::Session));
    let switch = |model: &str| Command::Configure {
        model: model.to_string(),
    };
    send(&a, "a-3", switch("b/n")).await;
    turn(&a, "a-4", "charlie").await;
    assert_eq!(
        last_sent(&first, "charlie"),
        "-",
        "b/n 没有 off，照配置：什么都不带"
    );
    assert_eq!(a.next().effort, None);
    send(&a, "a-5", switch("a/m")).await;
    turn(&a, "a-6", "delta").await;
    assert_eq!(last_sent(&first, "delta"), "none", "换回来又用上");
    assert_eq!(a.next().effort, used("off", EffortSource::Session));
    stop(&a).await;
    let loaded = home.load(a.id(), &routes).await;
    assert_eq!(
        loaded.next().effort,
        used("off", EffortSource::Session),
        "载入照日志算的造路由"
    );
    turn(&loaded, "a-7", "echo").await;
    assert_eq!(last_sent(&first, "echo"), "none");
}

/// 池不用会话记的那一档（SM-1 补，项目主人 10-30：「池子不应该能设置思考强度」）：在 `a/m` 上设了 `high`，换到只有 `a/m` 的池
/// 照配置的 `low` 发、`from` 是配置；换回 `a/m` 又用上 `high`。
#[tokio::test]
async fn a_pool_ignores_the_session_s_level() {
    let first = Server::start(hellos(24)).await;
    let mut home = Home::new();
    let system = format!("{}\n[pools.p]\nmodels = [\"a/m\"]\n", config(&first, "low"));
    home.configs = configs(&system, &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let a = create(&home, &routes, "a/m").await;
    send(&a, "a-1", effort(Some("high"))).await;
    turn(&a, "a-2", "alpha").await;
    assert_eq!(last_sent(&first, "alpha"), "high");
    let switch = |model: &str| Command::Configure {
        model: model.to_string(),
    };
    send(&a, "a-3", switch("@p")).await;
    turn(&a, "a-4", "bravo").await;
    assert_eq!(last_sent(&first, "bravo"), "low", "池照成员的配置");
    assert_eq!(a.next().effort, used("low", EffortSource::System));
    stop(&a).await;
    let a = home.load(a.id(), &routes).await;
    assert_eq!(
        a.next().effort,
        used("low", EffortSource::System),
        "载入时造的路由在池上也不用会话那一档"
    );
    send(&a, "a-5", switch("a/m")).await;
    turn(&a, "a-6", "charlie").await;
    assert_eq!(last_sent(&first, "charlie"), "high", "换回模型又用上");
}

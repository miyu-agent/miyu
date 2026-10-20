//! `check` 带语义模型的状态（施工 R-25，`docs/blueprint/protocol.md` 的 `check`）：真核心接上本机的小模型（手造的、真的
//! `miyu-embed`），记一条记忆、放一个知识库，`embedding` 写着用哪个模型、每个人格那一间和每个库补到了哪；关着的、没装的
//! 写为什么；没接照意思那一路的核心（测试里）不带；只查一份的不带。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::EmbedSetup;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::*;

fn tiny() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../miyu-session/tests/fixtures/embed-tiny")
}

/// cargo 编出来的测试用小程序 `miyu-test-embed`：和测试程序所在的 `deps/` 同一层。
fn program() -> PathBuf {
    let exe = std::env::current_exe().expect("知道测试程序在哪");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("在 target/<profile>/deps/ 里");
    dir.join(format!("miyu-test-embed{}", std::env::consts::EXE_SUFFIX))
}

/// 照系统配置 `system` 造核心，`local` 是本机的那一路（没有的是没装内置语义模型）。
fn core(home: &Home, system: &str, local: Option<EmbedSetup>) -> Arc<miyu_endpoint::Core> {
    home.write("system/config.toml", system);
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_models::settings::UseSettings::ITEMS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_vectors(local),
    )
}

fn local() -> EmbedSetup {
    EmbedSetup {
        program: Some(program()),
        manifest: tiny().join("manifest.toml"),
        dir: tiny(),
        idle: Duration::from_secs(600),
    }
}

async fn checked(client: &mut Client, id: &str, params: Value) -> Value {
    let reply = client.call(id, "check", params).await;
    reply["result"].clone()
}

const ENGINEER: &str = "[persona]\ndefault = \"engineer\"\n";

#[tokio::test]
async fn it_says_which_model_and_how_far_each_index_is_filled() {
    let home = Home::new();
    home.write("home/alice/kb/notes/cat.md", "# 团子\n\n我的猫，喝茶。\n");
    let mut client = Client::connect(core(&home, ENGINEER, Some(local())));
    client.hello().await;
    let reply = client
        .call(
            "r1",
            "memory.remember",
            json!({"class": "user", "text": "我的猫"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    // 搜一回知识库：照文件补索引，在后台补它的向量。
    let reply = client.call("k1", "kb.search", json!({"query": "猫"})).await;
    assert!(reply["result"].is_object(), "{reply}");
    let mut embedding = Value::Null;
    for n in 0..600 {
        embedding = checked(&mut client, &format!("c{n}"), json!({})).await["embedding"].clone();
        let full = |list: &Value| {
            list.as_array().is_some_and(|list| {
                !list.is_empty() && list.iter().all(|one| one["have"] == one["total"])
            })
        };
        if full(&embedding["memory"]) && full(&embedding["kb"]) && embedding["filling"] == false {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let model = embedding["model"].as_str().unwrap_or_default().to_string();
    assert!(model.starts_with("local:"), "本机的模型编号：{embedding}");
    assert_eq!(
        embedding,
        json!({
            "model": model,
            "filling": false,
            "memory": [{"persona": "engineer", "have": 1, "total": 1}],
            "turns": [{"persona": "engineer", "have": 0, "total": 0}],
            "kb": [{"id": "notes", "name": "notes", "have": 1, "total": 1}],
        }),
        "补齐了"
    );
    // 只查一份的不带。
    let settings = home.root.path().join("system").join("config.toml");
    let one = checked(
        &mut client,
        "f1",
        json!({"file": settings.to_string_lossy()}),
    )
    .await;
    assert_eq!(one, json!({"problems": []}), "{one}");
}

#[tokio::test]
async fn off_and_not_installed_say_why() {
    let home = Home::new();
    let mut client = Client::connect(core(
        &home,
        &format!("{ENGINEER}\n[models]\nembedding = \"off\"\n"),
        Some(local()),
    ));
    client.hello().await;
    let off = checked(&mut client, "c1", json!({})).await;
    assert_eq!(
        off["embedding"],
        json!({"model": null, "why": "off"}),
        "{off}"
    );

    let home = Home::new();
    let mut client = Client::connect(core(&home, ENGINEER, None));
    client.hello().await;
    let missing = checked(&mut client, "c1", json!({})).await;
    assert_eq!(
        missing["embedding"],
        json!({"model": null, "why": "not_installed"}),
        "{missing}"
    );
}

#[tokio::test]
async fn a_core_without_the_meaning_way_leaves_it_out() {
    let home = Home::new();
    let mut client = Client::connect(Arc::new(home.core_full(
        &Script::new([]),
        Catalog::default(),
        None,
        TOKEN,
    )));
    client.hello().await;
    let reply = checked(&mut client, "c1", json!({})).await;
    assert_eq!(reply, json!({"problems": []}), "{reply}");
}

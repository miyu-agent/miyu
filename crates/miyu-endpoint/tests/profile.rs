//! 「AI 怎么认识你」（施工 P-9、P-9 补，`docs/blueprint/prompts.md`「关于你」）：配置里写了称呼、更多描述、回复语言的，本机开的新会话的
//! system 里接上那一块、记进快照；没写的一字不变。场所会话只有没关起来（主人本人的私聊）、桥在 `venue.session` 里写了 `profile` 的
//! 才带。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::venues::BINDINGS;
use crate::support::*;

/// 写进系统配置的两项。
const PROFILE: &str = "[profile]\nname = \"小林\"\nabout = \"写 Rust 的独立开发者\"\n";

/// 拼出来的那一块。
const BLOCK: &str = "<user>\nname: 小林\nabout: 写 Rust 的独立开发者\n</user>";

/// 会话 `session` 的策略快照。
fn snapshot(home: &Home, session: &str) -> Snapshot {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs())
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

/// 照这时的配置起一个核心、握手、用命令编号 `command` 开一个会话，交回会话编号（编号要不一样：同一个编号核心当成重发，
/// 交回原来那个）。
async fn open(home: &Home, command: &str) -> String {
    let cwd = home.work.to_string_lossy().into_owned();
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client
        .call("h", "hello", json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"}, "caps": {"input": true}, "token": TOKEN}))
        .await;
    client.create(command, &cwd).await
}

#[tokio::test]
async fn a_written_profile_joins_the_system_of_new_sessions() {
    let home = Home::new();
    let plain = snapshot(&home, &open(&home, "c1").await);
    assert_eq!(plain.profile, None, "没写的没有");
    assert!(!plain.system.contains("<user>"), "{}", plain.system);
    home.write("config.toml", PROFILE);
    let with = snapshot(&home, &open(&home, "c2").await);
    let block = BLOCK;
    assert_eq!(with.profile.as_deref(), Some(block));
    assert!(
        with.system.contains(&format!("{block}\n\n")),
        "接在人设后面、核心的几行前面：{}",
        with.system
    );
    assert_eq!(
        with.system.replace(&format!("{block}\n\n"), ""),
        plain.system,
        "别的一字不变"
    );
}

#[tokio::test]
async fn a_reply_language_is_one_more_line_and_alone_still_joins() {
    let home = Home::new();
    home.write(
        "config.toml",
        &format!("{PROFILE}preferred_reply_language = \"简体中文\"\n"),
    );
    let both = snapshot(&home, &open(&home, "c1").await);
    assert_eq!(
        both.profile.as_deref(),
        Some(
            "<user>\nname: 小林\nabout: 写 Rust 的独立开发者\npreferred_reply_language: 简体中文\n</user>"
        ),
        "接在更多描述后面（施工 P-9 补）"
    );
    home.write(
        "config.toml",
        "[profile]\npreferred_reply_language = \"日本語\"\n",
    );
    let alone = snapshot(&home, &open(&home, "c2").await);
    assert_eq!(
        alone.profile.as_deref(),
        Some("<user>\npreferred_reply_language: 日本語\n</user>"),
        "只填了回复语言也带"
    );
}

/// 照对应表、「关于你」、读好的清单起来的核心，拉起开着的扩展（桥）。
fn served_core(home: &Home) -> Arc<Core> {
    home.write("config.toml", &format!("{BINDINGS}\n{PROFILE}"));
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_session::settings::ProfileSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root);
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 等到记下的回应有 `n` 行，交回它们。
async fn replies(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<Value> = read(path)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 会话 `session` 的快照里「关于你」那一块。
fn profile_of(home: &Home, session: &Value) -> Option<String> {
    let session = session["session"].as_str().expect("有编号");
    crate::support::venues::snapshot(home, &dir(home, session)).profile
}

#[tokio::test]
async fn venue_sessions_carry_it_only_unconfined_where_the_bridge_asks() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.session:{"venue":"qq:private:10001","kind":"private","peer":"qq:10001","profile":true}"#,
            r#"ask:venue.session:{"venue":"qq:private:10009","kind":"private","peer":"qq:10001"}"#,
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group","profile":true}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let _core = served_core(&home);
    let got = replies(&path, 4).await;
    assert_eq!(
        profile_of(&home, &got[1]["result"]).as_deref(),
        Some(BLOCK),
        "管理员本人的私聊、桥说要的：{got:?}"
    );
    assert_eq!(profile_of(&home, &got[2]["result"]), None, "桥没说要的");
    assert_eq!(
        profile_of(&home, &got[3]["result"]),
        None,
        "群归系统账号：说要也不带"
    );
}

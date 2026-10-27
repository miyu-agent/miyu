//! 每人一份的扩展（施工 IM-1 中上，接入通讯平台设计 06 第 4.6 节第 1 样）：清单写了 `[process] scope = "account"` 的包，核心
//! 起来时不起（头一回装上、没有登记缓存的先替机主起一份登记工具）；她替谁调到它的工具，才替那个人起一份，工作目录在他自己的家目录里，调用转给他那一份。管理员、勾了「成员也能用」
//! 的成员各起各的，谁的调用都不会落到别人那一份上。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ToolStatus};
use miyu_kernel::id::AccountId;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::login::member;
use crate::support::*;

/// 包 `id` 替 `account` 的那一份的工作目录。
fn state_of(home: &Home, account: &AccountId, id: &str) -> PathBuf {
    home.root
        .account_dir(account)
        .join("state")
        .join("packages")
        .join(id)
}

/// 会话目录 `dir` 里每一条工具结果的字和成没成。
fn results(dir: &Path) -> Vec<(String, bool)> {
    read_events(dir)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some((
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect(),
                result.status == ToolStatus::Ok,
            )),
            _ => None,
        })
        .collect()
}

/// 等到 `path` 里有 `want` 那一行。
async fn until_line(path: &Path, want: &str) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let text = read(path);
        if text.lines().any(|line| line.contains(want)) {
            return text;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {want}：{text}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn each_person_gets_their_own_process_when_they_first_use_it() {
    let home = Home::new();
    let program = Program::new();
    let tools = json!({"tools": [{"name": "ping", "description": "Ping.",
        "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]});
    install_providing(
        &home,
        "each",
        &program.name(),
        "always",
        &steps(&[
            "record:calls.log",
            "env:MIYU_PACKAGE_STATE",
            "hello",
            &format!("ask:provide:{tools}"),
            "serve",
        ]),
    );
    let manifest = home
        .root
        .system_packages()
        .join("each")
        .join("package.toml");
    let text = read(&manifest)
        .replacen("[package]\n", "[package]\nmembers = true\n", 1)
        .replacen("[process]\n", "[process]\nscope = \"account\"\n", 1);
    std::fs::write(&manifest, text).expect("写得进");
    let script = Script::new([
        Play::calls(&[("ping", "{}")]),
        Play::Says("通了。"),
        Play::calls(&[("ping", "{}")]),
        Play::Says("也通了。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let alice = AccountId::parse("alice").expect("合写法");
    let bob = AccountId::parse("bob").expect("合写法");
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    // 头一回装上、没有登记缓存：先替机主起一份，把工具登记上。
    let mine = state_of(&home, &alice, "each").join("calls.log");
    until_line(&mine, "\"tools\":1").await;
    let theirs = state_of(&home, &bob, "each").join("calls.log");
    assert!(!theirs.exists(), "别人的不起");

    // 管理员先用：替他起一份，工作目录在他家里。
    let session = owner.create("c1", &home.work.to_string_lossy()).await;
    owner.say("s1", &session, "试试").await;
    until_turns_in(&dir(&home, &alice, &session), 1).await;
    assert_eq!(
        results(&dir(&home, &alice, &session)),
        [("served ping".to_string(), true)]
    );
    let text = until_line(&mine, "tool.call").await;
    let real = std::fs::canonicalize(state_of(&home, &alice, "each")).expect("建了");
    assert!(
        text.contains(&format!("MIYU_PACKAGE_STATE={}", real.display())),
        "{text}"
    );
    assert!(!theirs.exists(), "没用过的人没有");

    // 成员再用：另起一份，调用落在他那一份上。
    let mut member = member(&core, "bob").await;
    let workspace = home.root.workspace(&bob);
    let his = member.create("c2", &workspace.to_string_lossy()).await;
    member.say("s2", &his, "试试").await;
    until_turns_in(&dir(&home, &bob, &his), 1).await;
    assert_eq!(
        results(&dir(&home, &bob, &his)),
        [("served ping".to_string(), true)]
    );
    until_line(&theirs, "tool.call").await;
    let calls = |path: &Path| {
        read(path)
            .lines()
            .filter(|line| line.contains("tool.call"))
            .count()
    };
    assert_eq!(calls(&mine), 1, "他的调用没落到管理员那一份上");
    assert_eq!(calls(&theirs), 1);
    core.stop_extensions().await;
}

/// 照 `miyu-core` 起来时那样拼进包的配置项的核心，模型照 `script` 答，照开关拉起扩展。
fn core_with(home: &Home, script: &Script) -> Arc<miyu_endpoint::Core> {
    let core_items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let alice = AccountId::parse("alice").expect("合写法");
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, &alice);
    let packaged = miyu_endpoint::packages::settle(&mut found, &core_items);
    let items = core_items.into_iter().chain(packaged).collect();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 每人一份的那一份握手拿到的是那个人自己的设置；登记缓存有了以后，核心再起来不先替机主起。
#[tokio::test]
async fn each_copy_gets_its_person_s_settings_and_a_cached_package_waits() {
    let home = Home::new();
    let program = Program::new();
    let tools = json!({"tools": [{"name": "ping", "description": "Ping.",
        "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]});
    install_providing(
        &home,
        "each",
        &program.name(),
        "always",
        &steps(&[
            "record:calls.log",
            "hello",
            &format!("ask:provide:{tools}"),
            "serve",
        ]),
    );
    let manifest = home
        .root
        .system_packages()
        .join("each")
        .join("package.toml");
    let text = read(&manifest)
        .replacen("[package]\n", "[package]\nmembers = true\n", 1)
        .replacen("[process]\n", "[process]\nscope = \"account\"\n", 1)
        + "\n[settings.note]\ntype = \"text\"\nlayers = [\"system\", \"personal\"]\nname = { en = \"Note\" }\n";
    std::fs::write(&manifest, text).expect("写得进");
    let alice = AccountId::parse("alice").expect("合写法");
    let bob = AccountId::parse("bob").expect("合写法");
    let mine = state_of(&home, &alice, "each").join("calls.log");
    let theirs = state_of(&home, &bob, "each").join("calls.log");

    let core = core_with(
        &home,
        &Script::new([Play::calls(&[("ping", "{}")]), Play::Says("通了。")]),
    );
    until_line(&mine, "\"tools\":1").await;
    let (mut member, token) = crate::support::login::member_with_token(&core, "bob").await;
    let set = member
        .call(
            "config-5a1e9c3b7d2f4e60-1",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "each.note", "value": "bob-note"}]}),
        )
        .await;
    assert!(set["result"].is_object(), "{set}");
    let workspace = home.root.workspace(&bob);
    let session = member.create("c1", &workspace.to_string_lossy()).await;
    member.say("s1", &session, "试试").await;
    until_turns_in(&dir(&home, &bob, &session), 1).await;
    let text = until_line(&theirs, "tool.call").await;
    assert!(text.contains("bob-note"), "握手拿到他自己的设置：{text}");
    assert!(!read(&mine).contains("bob-note"), "机主那一份拿不到他的");
    core.stop_extensions().await;
    drop(member);
    drop(core);

    // 再起来：登记缓存有了，不先替机主起；他用的时候照旧替他起。
    let core = core_with(
        &home,
        &Script::new([Play::calls(&[("ping", "{}")]), Play::Says("又通了。")]),
    );
    let mut member = Client::connect(Arc::clone(&core));
    crate::support::login::hello_with(&mut member, json!({"login": token})).await;
    let session = member.create("c2", &workspace.to_string_lossy()).await;
    member.say("s2", &session, "再试").await;
    until_turns_in(&dir(&home, &bob, &session), 1).await;
    assert_eq!(
        results(&dir(&home, &bob, &session)),
        [("served ping".to_string(), true)]
    );
    let starts = |path: &Path| {
        read(path)
            .lines()
            .filter(|line| line.starts_with("cwd:"))
            .count()
    };
    assert_eq!(starts(&theirs), 2, "他的起了两回");
    assert_eq!(starts(&mine), 1, "机主的只在头一回装上时起过");
    core.stop_extensions().await;
}

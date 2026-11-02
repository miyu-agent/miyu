//! 发布前第三轮安全审计（2026-11-02）的防御性测试：只断言「不该发生的没发生」，修之前是红的。
//!
//! 1. 扩展改自己的配置（`config/from_package.rs`）不收密钥类的项、不收写成引用的值：不然能把自己的项指到别人的密钥、核心环境
//!    里的 key，握手、`extension.config` 再交回真值。PC-2 加的「一组字段的列表」里的密钥字段漏了这一道（`holds_secret` 不往
//!    `List(Record)` 里看，值是数组不是对象、`input` 以 `[` 开头，三道都过）。
//! 2. `session.respond` 不写 `to`（O-64）只照事实开一轮，是「记事实 + 开一轮」；`session.note` 记事实要 `act_for_external`，
//!    只有 `sessions.drive` 的扩展不该能借 `session.respond` 往主人本机的会话里记事实、开一轮。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::extensions::*;
use crate::support::served::{replies, served_core};
use crate::support::*;

/// 一个带「账号列表」的扩展：每一项有备注和密钥字段。
fn accounts_manifest(program: &str, args: &[String]) -> String {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    format!(
        r#"[package]
protocol = [1, 1]
name = {{ en = "Echo" }}

[command]
name = "echo"
program = "{program}"
about = {{ en = "E" }}

[process]
args = [{}]
start = "manual"

[settings.accounts]
type = "list"
element = "record"
title = "name"
name = {{ en = "Accounts" }}

[settings.accounts.fields.name]
type = "text"
name = {{ en = "Name" }}

[settings.accounts.fields.key]
type = "secret"
name = {{ en = "Key" }}
"#,
        args.join(", ")
    )
}

#[tokio::test]
async fn audit_an_extension_cannot_point_a_record_secret_field_at_another_secret() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "echo");
    let by_value = format!(
        "ask:config.set:{}",
        json!({"layer": "system", "changes": [{"key": "echo.accounts", "value": [{"name": "a", "key": {"secret": "deepseek"}}]}]})
    );
    let by_input = format!(
        "ask:config.set:{}",
        json!({"layer": "system", "changes": [{"key": "echo.accounts", "input": "[{ name = \"b\", key = { env = \"DEEPSEEK_API_KEY\" } }]"}]})
    );
    home.write(
        "packages/echo/package.toml",
        &accounts_manifest(
            &program.name(),
            &steps(&[&keep, "hello", &by_value, &by_input, "listen"]),
        ),
    );
    // 主人给别的用途存的密钥：扩展不该拿得到。
    home.write("secrets.toml", "deepseek = \"sk-victim-secret\"\n");
    let core = core_with_settings(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    call(&mut client, "extension.enable", "echo").await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let answers = loop {
        let answers: Vec<Value> = read(&path)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|line| line.get("result").is_some() || line.get("error").is_some())
            .collect();
        // 握手一条，两次 config.set 两条。
        if answers.len() >= 3 {
            break answers;
        }
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    // 给推送一点时间：改成了的话 `extension.config` 会把真值推过来。
    tokio::time::sleep(Duration::from_millis(300)).await;
    let recorded = read(&path);
    let config = read(&home.root.config_file());
    core.stop_extensions().await;
    let reasons: Vec<Value> = answers[1..3]
        .iter()
        .map(|answer| answer["error"]["data"]["reason"].clone())
        .collect();
    assert_eq!(
        reasons,
        [json!("local_only"), json!("local_only")],
        "扩展把自己一组字段里的密钥字段指到了别的密钥、环境变量：{answers:?}\n系统配置：{config}\n扩展收到的：{recorded}"
    );
    assert!(
        !recorded.contains("sk-victim-secret"),
        "别的密钥的真值交给了扩展：{recorded}"
    );
}

/// 同上，换成网址的列表（施工 8-6b：网址能写成 `{ env = … }`）：值是数组、不是对象，`holds_secret` 也不看 `List(Url)`，扩展
/// 能把引用写进系统配置。现在 `handed` 不解列表元素里的引用，交回去的不是真值（第二条断言现在不红）；规矩（「写成引用的值不收」）
/// 照样破了，哪天 `handed` 往列表里解一层就成了泄露。基点 cc065472d 就有（列表元素能是 `url`），这一轮才查到。
#[tokio::test]
async fn audit_an_extension_cannot_point_a_url_list_at_the_core_environment() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "echo");
    let set = format!(
        "ask:config.set:{}",
        json!({"layer": "system", "changes": [{"key": "echo.mirrors", "value": [{"env": "AUDIT_CORE_KEY"}]}]})
    );
    let args: Vec<String> = steps(&[&keep, "hello", &set, "listen"])
        .iter()
        .map(|arg| format!("{arg:?}"))
        .collect();
    home.write(
        "packages/echo/package.toml",
        &format!(
            r#"[package]
protocol = [1, 1]
name = {{ en = "Echo" }}

[command]
name = "echo"
program = "{}"
about = {{ en = "E" }}

[process]
args = [{}]
start = "manual"

[settings.mirrors]
type = "list"
element = "url"
name = {{ en = "Mirrors" }}
"#,
            program.name(),
            args.join(", ")
        ),
    );
    // 核心环境里的 key（`{ env }` 照核心的环境取）。
    let core_items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root);
    let packaged = miyu_endpoint::packages::settle(&mut found, &core_items);
    let items = core_items.into_iter().chain(packaged).collect();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[("AUDIT_CORE_KEY", "sk-core-env-secret")]),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    call(&mut client, "extension.enable", "echo").await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let answers = loop {
        let answers: Vec<Value> = read(&path)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|line| line.get("result").is_some() || line.get("error").is_some())
            .collect();
        if answers.len() >= 2 {
            break answers;
        }
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    let recorded = read(&path);
    let config = read(&home.root.config_file());
    core.stop_extensions().await;
    assert_eq!(
        answers[1]["error"]["data"]["reason"], "local_only",
        "扩展把自己的网址列表指到了核心环境里的变量：{}\n系统配置：{config}\n扩展收到的：{recorded}",
        answers[1]
    );
    assert!(
        !recorded.contains("sk-core-env-secret"),
        "核心环境里的 key 交给了扩展：{recorded}"
    );
}

#[tokio::test]
async fn audit_facts_only_respond_needs_what_note_needs() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "plain");
    let local_session = home.work.join("local-session");
    install_asking(
        &home,
        "plain",
        &program.name(),
        "always",
        &steps(&[
            &keep,
            "hello",
            &format!("session:{}", local_session.display()),
            r#"ask:session.respond:{"session":"{session}","facts":[{"kind":"env","text":"AUDIT-INJECTED-FACT"}]}"#,
            "wait",
        ]),
        &["sessions.drive"],
    );
    approve(&home, "plain", &["sessions.drive"]);
    let core = served_core(&home, &Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let here = client.create("c1", "~").await;
    std::fs::write(&local_session, &here).expect("写得进");
    let got = replies(&path, 2).await;
    core.stop_extensions().await;
    assert!(
        got[1].get("error").is_some(),
        "只有 sessions.drive 的扩展借 session.respond 不写 to 往主人本机的会话里记了事实、开了一轮\
         （session.note 记事实要 act_for_external）：{}",
        got[1]
    );
}

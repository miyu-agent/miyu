//! 扩展拿主人配的某一家模型供应商的 key（施工 PK-1）：清单 `[provider_key] catalog` 写要哪一家、声明 `provider.key`、人批过的，
//! 握手的 `config` 多一格 `provider_key`（那一家的 key 的真值），换了 key、删了那一家照配置推送推；别家的 key 绝不交，没配那一家
//! 的不交这一格，没批的不拉起、拿不到；`config.get` 照旧只交引用。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;

use crate::support::extensions::*;
use crate::support::*;

/// 要 DeepSeek 的 key 的扩展：程序 `program`、参数 `args`，没有自己的配置项。
fn manifest(program: &str, args: &[String]) -> String {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Voice\" }}\n\n[command]\nname = \"voice\"\nprogram = \"{program}\"\nabout = {{ en = \"V\" }}\n\n[process]\nargs = [{}]\nstart = \"manual\"\ncapabilities = [\"provider.key\"]\n\n[provider_key]\ncatalog = \"deepseek\"\n",
        args.join(", ")
    )
}

/// 配了两家：照官方地址认得出是 DeepSeek 的 `ds`，和 OpenRouter 的 `or`。
const PROVIDERS: &str = "[providers.ds]\nbase_url = \"https://api.deepseek.com\"\nkey = { secret = \"ds\" }\n\n[providers.or]\ncatalog = \"openrouter\"\nkey = { secret = \"or\" }\n";

/// 两家的 key（假值）。
const SECRETS: &str = "ds = \"sk-ds-test-1\"\nor = \"sk-or-test-1\"\n";

/// 记下的每一行里的 JSON（`cwd:` 那一行除外）。
fn recorded(path: &std::path::Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// 等到记下的里有握手的回应，交回它。
async fn handshake(path: &std::path::Path) -> Value {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if let Some(hello) = recorded(path)
            .into_iter()
            .find(|line| line.get("result").is_some())
        {
            return hello;
        }
        assert!(tokio::time::Instant::now() < deadline, "{}", read(path));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// 等到记下的推送有 `n` 条，交回它们的 `params`。
async fn pushes(path: &std::path::Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let found: Vec<Value> = recorded(path)
            .into_iter()
            .filter(|line| line["method"] == "extension.config")
            .map(|line| line["params"].clone())
            .collect();
        if found.len() >= n {
            return found;
        }
        assert!(tokio::time::Instant::now() < deadline, "{}", read(path));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// 同 `core_with_settings`，配置清单另拼进供应商的配置项（测试用的核心原来不认 `providers.<id>`）。
fn core_with_providers(home: &Home) -> Arc<Core> {
    let core_items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_models::settings::ProviderSettings::ITEMS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, Vec::new());
    let packaged = miyu_endpoint::packages::settle(&mut found, &core_items);
    let items = core_items.into_iter().chain(packaged).collect();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(
            &miyu_session::testkit::Script::new([]),
            miyu_tool::Catalog::default(),
            None,
            TOKEN,
        )
        .with_extension_timing(quick())
        .with_config(config)
        .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 照 `providers`、`secrets` 起一个核心，装上要 DeepSeek 的 key 的扩展，带着批准打开：交回核心、连接、记下的文件。
async fn approved_and_running(
    home: &Home,
    program: &Program,
    providers: &str,
    secrets: &str,
) -> (Arc<Core>, Client, std::path::PathBuf) {
    let (path, keep) = record(home, "voice");
    home.write(
        "opt/voice/package.toml",
        &manifest(&program.name(), &steps(&[&keep, "hello", "listen"])),
    );
    home.write("etc/config.toml", providers);
    home.write("etc/private/secrets.toml", secrets);
    let core = core_with_providers(home);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = client
        .call(
            "e",
            "extension.enable",
            json!({"package": "voice", "approve": ["provider.key"]}),
        )
        .await;
    assert!(enabled.get("error").is_none(), "{enabled}");
    until_state(&mut client, "voice", |one| one["state"] == "running").await;
    (core, client, path)
}

#[tokio::test]
async fn an_approved_extension_gets_that_providers_key_and_its_changes() {
    let home = Home::new();
    let program = Program::new();
    let (core, mut client, path) = approved_and_running(&home, &program, PROVIDERS, SECRETS).await;
    let hello = handshake(&path).await;
    assert_eq!(
        hello["result"]["config"],
        json!({"provider_key": "sk-ds-test-1"}),
        "批过的拿到那一家的真值：{hello}"
    );

    let other = client
        .call(
            "s0",
            "secret.set",
            json!({"name": "or", "value": "sk-or-test-2"}),
        )
        .await;
    assert!(other.get("error").is_none(), "{other}");
    let changed = client
        .call(
            "s1",
            "secret.set",
            json!({"name": "ds", "value": "sk-ds-test-2"}),
        )
        .await;
    assert!(changed.get("error").is_none(), "{changed}");
    assert_eq!(
        pushes(&path, 1).await,
        [json!({"keys": {"provider_key": "sk-ds-test-2"}})],
        "换了那一家的 key 推新值，别家的 key 换了不推"
    );
    let removed = client
        .call(
            "c1",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "providers.ds.key", "unset": true}]}),
        )
        .await;
    assert!(removed.get("error").is_none(), "{removed}");
    let all = pushes(&path, 2).await;
    assert_eq!(
        all[1],
        json!({"keys": {"provider_key": null}}),
        "那一家没了 key 推 null"
    );
    let log = read(&path);
    assert!(
        !log.contains("sk-or-test"),
        "别家的 key 一个字都没交出去：{log}"
    );

    let got = client
        .call("g", "config.get", json!({"keys": ["providers.or.key"]}))
        .await;
    assert_eq!(
        got["result"]["items"]["providers.or.key"]["value"],
        json!({"secret": "or"}),
        "config.get 照旧只交引用：{got}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn no_cell_when_that_provider_is_not_configured() {
    let home = Home::new();
    let program = Program::new();
    let only_other = "[providers.or]\ncatalog = \"openrouter\"\nkey = { secret = \"or\" }\n";
    let (core, _client, path) = approved_and_running(&home, &program, only_other, SECRETS).await;
    let hello = handshake(&path).await;
    assert_eq!(
        hello["result"]["config"],
        json!({}),
        "没配 DeepSeek 的不交这一格，别家的也不交：{hello}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn an_unapproved_extension_does_not_start_and_gets_nothing() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "voice");
    home.write(
        "opt/voice/package.toml",
        &manifest(&program.name(), &steps(&[&keep, "hello", "listen"])),
    );
    home.write("etc/config.toml", PROVIDERS);
    home.write("etc/private/secrets.toml", SECRETS);
    let core = core_with_providers(&home);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let refused = call(&mut client, "extension.enable", "voice").await;
    assert_eq!(
        refused["error"]["data"]["reason"], "needs_approval",
        "{refused}"
    );
    assert_eq!(
        refused["error"]["data"]["capabilities"],
        json!(["provider.key"])
    );
    let voice = status(&mut client, "voice").await;
    assert_ne!(voice["state"], "running", "{voice}");
    assert!(!read(&path).contains("sk-"), "没批的一把 key 都拿不到");
    core.stop_extensions().await;
}

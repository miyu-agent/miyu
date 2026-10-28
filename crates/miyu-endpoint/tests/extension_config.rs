//! 握手时把包自己的配置交给它（施工 9-4 下下，`docs/blueprint/extensions.md`「配置」）：核心拉起的扩展握手的回应带
//! `config`（这个包自己的键、最终值、密钥是真值、没设的不放），之后它的键变了推 `extension.config`（只放变了的，没了的是
//! `null`），别的键变了不推；头的连接没有这一格。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::*;

/// 一个带三项配置的扩展：端口有默认值，令牌是密钥，备注没默认值。
fn manifest(program: &str, args: &[String]) -> String {
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

[settings.port]
type = "int"
default = 8400
layers = ["system"]
name = {{ en = "Port" }}

[settings.token]
type = "secret"
layers = ["system"]
name = {{ en = "Token" }}

[settings.note]
type = "text"
layers = ["system"]
name = {{ en = "Note" }}
"#,
        args.join(", ")
    )
}

/// 记下的每一行里的 JSON（`cwd:` 那一行除外）。
fn recorded(path: &std::path::Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
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

#[tokio::test]
async fn the_handshake_hands_over_its_own_settings_and_changes_follow() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "echo");
    home.write(
        "packages/echo/package.toml",
        &manifest(&program.name(), &steps(&[&keep, "hello", "listen"])),
    );
    home.write("config.toml", "[echo]\ntoken = { secret = \"echo\" }\n");
    home.write("secrets.toml", "echo = \"s3cret\"\n");
    let core = core_with_settings(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    let mine = client.hello().await;
    assert!(
        mine["result"].get("config").is_none(),
        "头的连接没有这一格：{mine}"
    );

    call(&mut client, "extension.enable", "echo").await;
    until_state(&mut client, "echo", |one| one["state"] == "running").await;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    let hello = loop {
        if let Some(hello) = recorded(&path)
            .into_iter()
            .find(|line| line.get("result").is_some())
        {
            break hello;
        }
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    assert_eq!(
        hello["result"]["config"],
        json!({"echo.port": 8400, "echo.token": "s3cret"}),
        "默认值照给、密钥是真值、没设的不放：{hello}"
    );

    let not_mine = client
        .call(
            "c1",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "ui.language", "value": "en"}]}),
        )
        .await;
    assert!(not_mine.get("error").is_none(), "{not_mine}");
    let port = client
        .call(
            "c2",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "echo.port", "value": 9000}]}),
        )
        .await;
    assert!(port.get("error").is_none(), "{port}");
    assert_eq!(
        pushes(&path, 1).await,
        [json!({"keys": {"echo.port": 9000}})]
    );
    let secret = client
        .call("s1", "secret.set", json!({"name": "echo", "value": "n3w"}))
        .await;
    assert!(secret.get("error").is_none(), "{secret}");
    assert_eq!(
        pushes(&path, 2).await[1],
        json!({"keys": {"echo.token": "n3w"}}),
        "只换了引用的密钥的值也推"
    );
    let unset = client
        .call(
            "c3",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "echo.token", "unset": true}]}),
        )
        .await;
    assert!(unset.get("error").is_none(), "{unset}");
    let all = pushes(&path, 3).await;
    assert_eq!(
        all[2],
        json!({"keys": {"echo.token": null}}),
        "没了的是 null"
    );
    assert_eq!(all.len(), 3, "别的键变了不推：{all:?}");
    core.stop_extensions().await;
}

/// 扩展改自己的配置（施工 T-9 补，接入QQ 的会话 2026-10-17 要的、项目主人定）：核心拉起的扩展只能改系统配置里、键以自己的
/// 包编号开头的几项，逐项改；改了留痕记成这个包。密钥类的项、写成引用的值、别的包的键、整份换的照旧 `local_only`：
/// 握手时它拿到的密钥是真值，能改自己的密钥引用就能指到别人的 key 再读回去。
#[tokio::test]
async fn an_extension_sets_only_its_own_plain_settings() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "echo");
    let set = |changes: Value| {
        format!(
            "ask:config.set:{}",
            json!({"layer": "system", "changes": changes})
        )
    };
    let asks = [
        set(json!([{"key": "echo.port", "value": 9000}])),
        set(json!([{"key": "echo.token", "value": {"secret": "deepseek"}}])),
        set(json!([{"key": "echo.note", "value": {"env": "DEEPSEEK_API_KEY"}}])),
        set(json!([{"key": "echo.note", "input": "{ env = \"DEEPSEEK_API_KEY\" }"}])),
        set(json!([{"key": "ui.language", "value": "en"}])),
        format!(
            "ask:config.set:{}",
            json!({"layer": "system", "text": "[echo]\nport = 1\n", "version": null})
        ),
    ];
    let mut plan: Vec<&str> = vec![&keep, "hello"];
    plan.extend(asks.iter().map(String::as_str));
    plan.push("listen");
    home.write(
        "packages/echo/package.toml",
        &manifest(&program.name(), &steps(&plan)),
    );
    let core = core_with_settings(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    call(&mut client, "extension.enable", "echo").await;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    let answers = loop {
        let answers: Vec<Value> = recorded(&path)
            .into_iter()
            .filter(|line| line.get("result").is_some() || line.get("error").is_some())
            .collect();
        // 握手一条，六次 config.set 六条。
        if answers.len() >= 7 {
            break answers;
        }
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    assert!(
        answers[1].get("error").is_none(),
        "自己的项照改：{}",
        answers[1]
    );
    for refused in &answers[2..7] {
        assert_eq!(
            refused["error"]["data"]["reason"], "local_only",
            "{refused}"
        );
    }
    let journal = read(&home.root.path().join("journal.jsonl"));
    let changed = journal
        .lines()
        .find(|line| line.contains("echo.port"))
        .unwrap_or_else(|| panic!("{journal}"));
    assert!(
        changed.contains(r#""by":{"kind":"module","id":"echo"}"#),
        "留痕记成这个包：{changed}"
    );
    assert!(
        !read(&home.root.path().join("config.toml")).contains("deepseek"),
        "引用一个都没写进去"
    );
    core.stop_extensions().await;
}

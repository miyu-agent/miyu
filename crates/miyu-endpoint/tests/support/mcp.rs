//! MCP 服务的测试共用的（施工 X-3 上、下）：测试用的服务写成 `bundle` 包、等状态、造会话、看工具面和结果。`tests/mcp.rs`、
//! `tests/mcp_cache.rs`、`tests/mcp_settings_status.rs`、`tests/mcp_edit.rs` 用。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ToolStatus};
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use super::extensions::{quick, read};
use super::{Client, Home, TOKEN};

/// 测试用的服务。
pub const SERVER: &str = env!("CARGO_BIN_EXE_miyu-test-mcp");

/// 一个 `bundle` 包 `id`，带一个服务 `service`：命令是测试用的服务和 `args`，环境变量 `MIYU_MCP_PROBE` 设成 `set`，一次
/// 调用等 1 秒。路径写成 TOML 的字面字符串：Windows 的反斜杠不用转义。
pub fn bundle(home: &Home, id: &str, service: &str, args: &[&str]) {
    let command: Vec<String> = std::iter::once(SERVER)
        .chain(args.iter().copied())
        .map(|part| format!("'{part}'"))
        .collect();
    home.write(
        &format!("packages/{id}/package.toml"),
        &format!(
            "[package]\nname = {{ en = \"{id}\" }}\n\n[[mcp]]\nname = \"{service}\"\ncommand = [{}]\nenv = {{ MIYU_MCP_PROBE = \"set\" }}\ntimeout_ms = 1000\nneed_load = false\n",
            command.join(", ")
        ),
    );
}

/// 记收到的每一行的参数：文件在工作目录里。
pub fn recording(home: &Home, service: &str) -> (std::path::PathBuf, String) {
    let path = home.work.join(format!("mcp-{service}"));
    let arg = format!("record:{}", path.display());
    (path, arg)
}

/// `package.list` 里编号是 `id` 的那一项。
pub async fn entry(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

/// 等到包 `id` 的状态是 `status`：最多 60 秒。
pub async fn until_status(client: &mut Client, id: &str, status: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let one = entry(client, id).await;
        if one["status"] == status {
            return one;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {status}：{one}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 等到 `path` 里有一行含 `needle`：最多 60 秒。
pub async fn until_line(path: &Path, needle: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !read(path).contains(needle) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {needle}：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 开包 `id`，交回回应。
pub async fn enable(client: &mut Client, id: &str) -> Value {
    client
        .call("e", "package.enable", json!({"package": id}))
        .await
}

/// 造一个完全放开的会话（`network` 的工具不问人）。
pub async fn open(client: &mut Client) -> String {
    let session = client.create("c1", "~").await;
    let set = client
        .call(
            "p",
            "session.set_permission_level",
            json!({"session": session, "level": "full"}),
        )
        .await;
    assert_eq!(set["result"], json!({}), "{set}");
    session
}

/// 第 `n` 次请求带的工具面里 MCP 的那几件。
pub fn face(script: &Script, n: usize) -> Vec<String> {
    script.requests()[n]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .filter(|name| name.starts_with("mcp__"))
        .collect()
}

/// 日志里每一条工具结果：工具名、字、出没出错、几张图，照工具名排（只读的几件一起跑，先到先记）。
pub fn results(home: &Home, session: &str) -> Vec<(String, String, bool, usize)> {
    let mut names = std::collections::BTreeMap::new();
    let mut found = Vec::new();
    for event in home.log(session) {
        match event.body {
            Body::MessageAssistant(message) => {
                for block in message.blocks {
                    if let Block::ToolCall(call) = block {
                        names.insert(call.call_id, call.name);
                    }
                }
            }
            Body::ToolResult(result) => {
                let text: String = result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect();
                let images = result
                    .blocks
                    .iter()
                    .filter(|block| matches!(block, Block::Image(_)))
                    .count();
                let name = names.get(&result.call_id).cloned().unwrap_or_default();
                found.push((name, text, result.status != ToolStatus::Ok, images));
            }
            _ => {}
        }
    }
    found.sort();
    found
}

/// `mcp.list` 里叫 `name` 的那一项（施工 F-9 补；施工 X-26 从 `tests/mcp_settings_status.rs` 挪来共用）。
pub async fn listed(client: &mut Client, name: &str) -> Value {
    let reply = client.call("l", "mcp.list", json!({})).await;
    reply["result"]["servers"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["name"] == name)
        .cloned()
        .unwrap_or_else(|| panic!("没有 {name}：{reply}"))
}

/// 等到 `mcp.list` 里 `name` 那一项合 `wanted`：最多 60 秒。
pub async fn until_listed(
    client: &mut Client,
    name: &str,
    wanted: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let one = listed(client, name).await;
        if wanted(&one) {
            return one;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到：{one}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 个人设置里一张 `[mcp.probe]`：测试用的服务，记下收到的每一行到 `record`，环境变量 `MIYU_MCP_PROBE` 设成 `mine`。
pub fn mine(record: &str) -> String {
    format!(
        "[mcp.probe]\ncommand = ['{SERVER}', '{record}']\nenv = {{ MIYU_MCP_PROBE = \"mine\" }}\ntimeout_ms = 1000\n"
    )
}

/// 照磁盘上的配置起来的核心，扩展退避得快。
pub fn configured(home: &Home, script: &Script) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_extension_timing(quick()),
    )
}

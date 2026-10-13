//! 个人设置里的 MCP 服务给头的方法（施工 F-9 补，`docs/blueprint/protocol.md` 的 `mcp.*`）：`mcp.list` 列出这时的状态，
//! `mcp.restart` 重启一个；状态变了推 `mcp.changed`（订阅软件包列表的连接收，`subscriptions/listed.rs`）。一项同软件包列表上
//! 服务的那一格（施工 X-4），另带 `covers`：它盖住了哪些包里的同名服务。只给本机的人用。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::McpServer;
use miyu_store::scripts::HOME_PACKAGE;

use crate::Core;
use crate::extensions::methods::details;
use crate::refusal::Refusal;

/// `mcp.restart` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RestartParams {
    /// 服务名：个人设置里的表名。
    name: String,
}

/// `mcp.list`：`{"servers": [...]}`，照个人设置里写的先后。
pub(crate) fn list(core: &Core) -> Value {
    let servers: Vec<Value> = core
        .home_mcp
        .now()
        .iter()
        .map(|server| one(core, server))
        .collect();
    json!({ "servers": servers })
}

/// 叫 `name` 的那一项；个人设置里没有了的没有（推送写 `null`）。
pub(crate) fn entry(core: &Core, name: &str) -> Option<Value> {
    core.home_mcp
        .now()
        .iter()
        .find(|server| server.name == name)
        .map(|server| one(core, server))
}

/// `mcp.restart`：停下、再起，连续失败从零数，工具留在目录里、起好列完照新的换；回应是重启以后的那一项。没有这个名字的
/// `unknown_server`。
pub(crate) async fn restart(core: &Arc<Core>, params: RestartParams) -> Result<Value, Refusal> {
    // 没有这个名字的：停不了、起的时候也没有它，什么都不动，下面拒。
    core.restart_home_mcp(&params.name).await;
    entry(core, &params.name).ok_or(Refusal::UNKNOWN_SERVER)
}

/// 一项：名字、命令、给哪种会话、状态（同软件包列表上服务的那一格），盖住了哪些包里的同名服务。
fn one(core: &Core, server: &McpServer) -> Value {
    let status = core.mcp.service(HOME_PACKAGE, &server.name);
    let mut one = json!({
        "name": server.name,
        "command": server.command,
        "venues": server.venues,
        "state": status.state.as_str(),
        "failures": status.failures,
        "covers": super::home::carrying(core, &server.name),
    });
    details(
        &mut one,
        status.state,
        &super::stderr_file(core, &server.name),
    );
    one
}

//! 配置里的 MCP 服务给头的方法（施工 F-9 补，`docs/blueprint/protocol.md` 的 `mcp.*`）：`mcp.list` 列出这时的状态，
//! `mcp.restart` 重启一个；状态变了推 `mcp.changed`（订阅软件包列表的连接收，`subscriptions/listed.rs`）。一项同软件包列表上
//! 服务的那一格（施工 X-4），另带 `covers`：它盖住了哪些包里的同名服务；知道有哪些工具的另带 `tools`（施工 X-26）；加载方式
//! `need_load` 和全局开关 `on`（施工 X-27）；浏览器登录过的带 `logged_in`（施工 X-42 下再补）。只给本机的人用。加、换、删在
//! `editing.rs`（施工 X-26）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::{EnvValue, MCP_TIMEOUT_DEFAULT, McpServer, Reach};
use miyu_store::scripts::HOME_PACKAGE;
use miyu_tool::mcp_prefix;

use crate::Core;
use crate::extensions::methods::details;
use crate::refusal::Refusal;

/// `mcp.restart` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RestartParams {
    /// 服务名：配置里的表名。
    name: String,
}

/// `mcp.list`：`{"servers": [...]}`，照配置里写的先后。
pub(crate) fn list(core: &Core) -> Value {
    let servers: Vec<Value> = core
        .home_mcp
        .now()
        .iter()
        .map(|server| one(core, server))
        .collect();
    json!({ "servers": servers })
}

/// 叫 `name` 的那一项；配置里没有了的没有（推送写 `null`）。
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

/// 一项：名字、命令、给哪种会话、状态（同软件包列表上服务的那一格），盖住了哪些包里的同名服务，知道了的另带工具，浏览器登录过的
/// 带 `logged_in`（施工 X-42 下再补）。
/// `on` 是全局开关 `features.mcp-<服务名>`，没写的算开。配置照最近换上的那一份读，不拿配置服务的锁：推 `mcp.changed` 时也
/// 调它。
fn one(core: &Core, server: &McpServer) -> Value {
    let status = core.mcp.service(HOME_PACKAGE, &server.name);
    let mut one = json!({
        "name": server.name,
        "venues": server.venues,
        "state": status.state.as_str(),
        "failures": status.failures,
        "covers": super::home::carrying(core, &server.name),
    });
    reach(server, &mut one);
    written(server, &mut one);
    toggles(core, server, &mut one);
    logged_in(core, &server.name, &mut one);
    details(
        &mut one,
        status.state,
        &super::stderr_file(core, &server.name),
    );
    let tools = tools(core, &server.name);
    if !tools.is_empty() {
        one["tools"] = json!(tools);
    }
    skipped(core, HOME_PACKAGE, &server.name, &mut one);
    one
}

/// 这一次列完没登记的（施工 X-37）：`skipped: [{"tool", "why"}]`，提示词是 `{"prompt", "why"}`（施工 X-39），没有的不写这一格。
/// 软件包列表上包带的服务同样。
pub(crate) fn skipped(core: &Core, package: &str, service: &str, one: &mut Value) {
    let skipped = core.mcp.skipped(package, service);
    if !skipped.is_empty() {
        one["skipped"] = skipped
            .iter()
            .map(|skipped| match skipped.prompt {
                true => json!({"prompt": skipped.name, "why": skipped.why}),
                false => json!({"tool": skipped.name, "why": skipped.why}),
            })
            .collect();
    }
}

/// 怎么到它：命令的带 `command`（程序加参数），网址的带 `url`（施工 X-32 上）。软件包列表上包带的服务同样。
pub(crate) fn reach(server: &McpServer, one: &mut Value) {
    match &server.reach {
        Reach::Command { command, .. } => one["command"] = json!(command),
        Reach::Url { url, .. } => one["url"] = json!(url),
    }
}

/// 编辑页要先填上原来写的（施工 X-27 补，网页的 MCP 页要）：`env` 照写的（字照原样，引用写成 `{"secret": 名字}`、`{"env": 变量}`，
/// 只是名字、不是值），没写的不带；`timeout_ms` 不是默认的才带。`command` 照旧是程序加参数，`mcp.set` 的 `command`、`args` 照它拆。
/// 网址的服务（施工 X-32 上）带 `url`，`bearer`、`headers` 照 `env` 的写法。字照原样给：它本来就明文写在配置里，只有本机的
/// 人列得到；要藏的值该写成密钥的引用。
fn written(server: &McpServer, one: &mut Value) {
    match &server.reach {
        Reach::Command { env, .. } => {
            if !env.is_empty() {
                one["env"] = as_written(env);
            }
        }
        Reach::Url {
            bearer, headers, ..
        } => {
            if let Some(bearer) = bearer {
                one["bearer"] = value_written(bearer);
            }
            if !headers.is_empty() {
                one["headers"] = as_written(headers);
            }
        }
    }
    if server.timeout_ms != MCP_TIMEOUT_DEFAULT {
        one["timeout_ms"] = json!(server.timeout_ms);
    }
}

/// 名字到值的一张表，值照 [`value_written`]。
fn as_written(pairs: &[(String, EnvValue)]) -> Value {
    Value::Object(
        pairs
            .iter()
            .map(|(name, value)| (name.clone(), value_written(value)))
            .collect(),
    )
}

/// 一个值照写的：字照原样，引用写成 `{"secret": 名字}`、`{"env": 变量}`。
fn value_written(value: &EnvValue) -> Value {
    match value {
        EnvValue::Text(text) => json!(text),
        EnvValue::Reference(reference) => {
            let (key, name) = reference.parts();
            json!({ key: name })
        }
    }
}

/// 给服务 `server` 的那一格填上 `on`（施工 X-27；软件包列表上包带的服务同样，X-27 补）。`need_load` 那一格施工 FL-1 拆了：
/// 工具一律全量。配置照最近换上的那一份读，不拿配置服务的锁：推送时也调它。
pub(crate) fn toggles(core: &Core, server: &McpServer, one: &mut Value) {
    let values = core.config_now().borrow().resolved().values();
    let switches = miyu_session::switches::of(&values);
    one["on"] = json!(miyu_session::switches::on(&switches, &server.feature_id()));
}

/// 浏览器登录过的（施工 X-42 下再补）：存着 `mcp-oauth-<服务名>` 的带 `logged_in: true`，没有的不写这一格，头照它把「登录」换成
/// 「登出」。只看密钥文件里有没有这个名字，不读值；配置照最近换上的那一份读（同 [`toggles`]），推送时也调它。
fn logged_in(core: &Core, service: &str, one: &mut Value) {
    let name = super::login::secret_name(service);
    if core.config_now().borrow().secrets.has(&name) {
        one["logged_in"] = json!(true);
    }
}

/// 服务 `service` 这时在工具目录里的工具（施工 X-26），照目录的先后：起好列完的、核心起来时照缓存先登记的都算（同 `preset.get`
/// 里 `mcp-<名字>` 那个功能的工具）。还不知道的（没起过、缓存也没有）是空的。
fn tools(core: &Core, service: &str) -> Vec<String> {
    let prefix = mcp_prefix(service);
    let catalog = core.tools();
    catalog
        .specs()
        .map(|spec| spec.name.as_str())
        .filter(|name| catalog.package_of(name) == Some(HOME_PACKAGE) && name.starts_with(&prefix))
        .map(str::to_string)
        .collect()
}

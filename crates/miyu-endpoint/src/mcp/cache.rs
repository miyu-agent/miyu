//! 目录缓存（施工 X-3 下，`mcp.md`「怎么走」第一条第 4 条、第三条第 1 条）：列完的写进 `state/mcp/<服务名>.json`，记着起它的
//! 命令、环境变量的写法、认出来的时代、每件工具的原文；一样的不写。核心起来、开服务时，命令和环境变量的写法没变的照它先登记
//! （服务还没好时调到的说暂时不可用），连的时候照记着的时代直接说。派生的数据：读不成、写不成的记一行 `WARN`，等服务自己列。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use miyu_config::package::{EnvValue, McpServer, Reach};
use miyu_mcp::{Era, Tool};
use miyu_store::generated;

use super::TARGET;
use crate::Core;

/// 缓存的写法。
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Cached {
    /// 起它的命令，原样。
    command: Vec<String>,
    /// 环境变量的写法：`名字=字` 或者 `名字={ secret = … }` 这样的引用，不是取出来的值。
    env: Vec<String>,
    /// 认出来的时代和版本。
    era: Era,
    /// 每件工具的原文。
    tools: Vec<Value>,
    /// 服务给模型的说明（施工 X-39）：没给的不写，以前的缓存没有这一格照样认得。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    /// 服务声明了资源（施工 X-41）：没声明的不写，以前的缓存没有这一格的照没声明。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    resources: bool,
}

/// 缓存里记着的（施工 X-39 起多了说明）。
#[derive(Debug, PartialEq)]
pub(super) struct Remembered {
    /// 认出来的时代和版本。
    pub(super) era: Era,
    /// 读得成的工具。
    pub(super) tools: Vec<Tool>,
    /// 服务给模型的说明，没给的没有。
    pub(super) instructions: Option<String>,
    /// 服务声明了资源（施工 X-41）。
    pub(super) resources: bool,
}

/// 服务 `service` 的缓存在哪。
pub(super) fn path(core: &Core, service: &str) -> PathBuf {
    core.root
        .state()
        .join("mcp")
        .join(format!("{service}.json"))
}

/// 缓存照什么认这个服务（施工 X-3 下、X-32 上）：命令的是命令和环境变量的写法（引用的照引用写，不取值）；网址的是网址和请求头的
/// 名字（有令牌的另记一个 `Authorization`），不认值：换了令牌，目录照旧能用。记在缓存文件原来的两格里，以前的缓存照旧认得。
fn key(server: &McpServer) -> (Vec<String>, Vec<String>) {
    match &server.reach {
        Reach::Command { command, env } => (
            command.clone(),
            env.iter()
                .map(|(name, value)| match value {
                    EnvValue::Text(text) => format!("{name}={text}"),
                    EnvValue::Reference(reference) => format!("{name}={}", reference.toml()),
                })
                .collect(),
        ),
        Reach::Url {
            url,
            bearer,
            headers,
        } => (
            vec![url.clone()],
            bearer
                .as_ref()
                .map(|_| "Authorization".to_string())
                .into_iter()
                .chain(headers.iter().map(|(name, _)| name.clone()))
                .collect(),
        ),
    }
}

/// 记下服务 `server` 这一次认出来的时代 `era`、列出来的 `tools`、给模型的说明 `instructions`（施工 X-39）、声明了资源没有
/// `resources`（施工 X-41）。
pub(super) fn write(
    core: &Core,
    server: &McpServer,
    era: &Era,
    (tools, instructions, resources): (&[Tool], Option<&str>, bool),
) {
    let (command, env) = key(server);
    let cached = Cached {
        command,
        env,
        era: era.clone(),
        tools: tools.iter().map(|tool| tool.raw.clone()).collect(),
        instructions: instructions.map(str::to_string),
        resources,
    };
    let path = path(core, &server.name);
    let written = serde_json::to_vec_pretty(&cached)
        .map_err(std::io::Error::other)
        .and_then(|bytes| generated::write_inside(&path, &bytes));
    if let Err(error) = written {
        tracing::warn!(target: TARGET, service = server.name.as_str(), file = %path.display(), error = %error, "mcp tools not cached");
    }
}

/// 服务 `server` 的缓存：命令、环境变量的写法和它记着的一样才算，交回时代、工具（读不成的那一件不要）、说明。没有的、对不上的
/// 没有；读不成的记一行。
pub(super) fn read(core: &Core, server: &McpServer) -> Option<Remembered> {
    read_at(&path(core, &server.name), server)
}

/// 同 [`read`]，缓存在 `path`。
fn read_at(path: &Path, server: &McpServer) -> Option<Remembered> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(target: TARGET, service = server.name.as_str(), file = %path.display(), error = %error, "mcp tools cache unreadable");
            return None;
        }
    };
    let cached = match serde_json::from_slice::<Cached>(&bytes) {
        Ok(cached) => cached,
        Err(error) => {
            tracing::warn!(target: TARGET, service = server.name.as_str(), file = %path.display(), error = %error, "mcp tools cache unreadable");
            return None;
        }
    };
    if (cached.command.clone(), cached.env.clone()) != key(server) {
        tracing::info!(target: TARGET, service = server.name.as_str(), "mcp tools cache outdated");
        return None;
    }
    let tools = cached
        .tools
        .iter()
        .filter_map(|raw| Tool::read(raw).ok())
        .collect();
    Some(Remembered {
        era: cached.era,
        tools,
        instructions: cached.instructions,
        resources: cached.resources,
    })
}

/// 卸包时清掉服务 `service` 的缓存和工作目录（施工 X-3 下：卸就是清干净），登录存的那一份（施工 X-42 上；配置里还有同名
/// 服务的不清：那是它的）。本来就没有的不算错，删不成的记一行。
pub(crate) fn forget(core: &Core, service: &str) {
    if !core
        .home_mcp
        .now()
        .iter()
        .any(|server| server.name == service)
    {
        crate::secrets::discard(core, &super::login::secret_name(service));
    }
    let cache = path(core, service);
    if let Err(error) = std::fs::remove_file(&cache)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(target: TARGET, service, file = %cache.display(), error = %error, "mcp tools cache not removed");
    }
    let dir = super::launch::work_dir(core, service);
    if let Err(error) = std::fs::remove_dir_all(&dir)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(target: TARGET, service, dir = %dir.display(), error = %error, "mcp dir not removed");
    }
}

#[cfg(test)]
mod tests;

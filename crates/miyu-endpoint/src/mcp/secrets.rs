//! 密钥变了，引用它的服务跟着重起（施工 X-32 补，`docs/blueprint/mcp.md`「个人设置里的一张表」第 4 条，2026-10-21 项目主人定）：
//! 先配了服务、后存令牌的，存完就连得上，不用重启核心、不用点重启。
//!
//! - 怎么醒：不轮询。`secret.set`、`secret.delete` 落了盘、手改密钥文件被看到，配置服务都换上新的一份，跟着个人设置的那个
//!   循环（`home.rs` 的 `follow_home_mcp`）本来就是被这一下叫醒的；醒来时拿这一刻每个密钥值的摘要（[`prints`]）和上一刻的比，
//!   变了的名字就是要重起的。摘要只在内存里，不落盘、不进日志。
//! - 谁重起：`env`、`bearer`、`headers` 里写了 `{ secret = <变了的名字> }` 的服务，个人设置里的、包里带的都算；只重起开着的
//!   （看管的那一格不是关着的：在跑、在起、在等、连不上停下了的），人关掉的、包没开的不因此被拉起来。包里带的照
//!   `package.restart` 那条路，连同这个包的别的服务一起。
//! - 多用户：现在只有管理员一个账号、一份密钥文件，照它做；多用户那一段照账号分开（`24-威胁模型.md`）。
//! - 重起时记一行 `INFO mcp restarted … why="secret changed" secret=<名字>`，只有名字。

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use miyu_config::package::{EnvValue, McpServer, Reach};
use miyu_config::secret::Reference;
use miyu_store::scripts::HOME_PACKAGE;

use super::TARGET;
use crate::Core;
use crate::config::Config;
use crate::extensions::State;

/// 这一刻每个密钥值的摘要：名字到摘要。只拿来和上一刻比。
pub(super) fn prints(config: &Config) -> BTreeMap<String, u64> {
    config
        .secrets
        .stored
        .entries
        .iter()
        .map(|(name, secret)| {
            let mut hasher = DefaultHasher::new();
            secret.expose().hash(&mut hasher);
            (name.clone(), hasher.finish())
        })
        .collect()
}

/// 两刻之间变了的名字：新存的、删掉的、值改了的。
pub(super) fn changed(
    before: &BTreeMap<String, u64>,
    now: &BTreeMap<String, u64>,
) -> BTreeSet<String> {
    before
        .keys()
        .chain(now.keys())
        .filter(|name| before.get(*name) != now.get(*name))
        .cloned()
        .collect()
}

/// 服务 `server` 引用了 `names` 里的哪一个：`env`、`bearer`、`headers` 里写的 `{ secret = … }`。没引用的没有。
fn uses<'a>(server: &McpServer, names: &'a BTreeSet<String>) -> Option<&'a str> {
    let values: Vec<&EnvValue> = match &server.reach {
        Reach::Command { env, .. } => env.iter().map(|(_, value)| value).collect(),
        Reach::Url {
            bearer, headers, ..
        } => bearer
            .iter()
            .chain(headers.iter().map(|(_, value)| value))
            .collect(),
    };
    values.into_iter().find_map(|value| match value {
        EnvValue::Reference(Reference::Secret(name)) => names.get(name).map(String::as_str),
        _ => None,
    })
}

impl Core {
    /// 密钥 `changed` 变了：引用了它们、开着的服务停下再起，连续失败从零数。
    pub(super) async fn follow_secrets(self: &Arc<Self>, changed: &BTreeSet<String>) {
        for server in self.home_mcp.now().iter() {
            let Some(secret) = uses(server, changed) else {
                continue;
            };
            if self.mcp.service(HOME_PACKAGE, &server.name).state == State::Off {
                continue;
            }
            tracing::info!(target: TARGET, package = HOME_PACKAGE, service = server.name.as_str(), why = "secret changed", secret, "mcp restarted");
            self.restart_home_mcp(&server.name).await;
        }
        for found in self.packages().iter() {
            let Ok(manifest) = &found.read else {
                continue;
            };
            let touched = manifest.mcp.iter().find_map(|server| {
                let secret = uses(server, changed)?;
                (self.mcp.service(&found.id, &server.name).state != State::Off)
                    .then_some((server.name.as_str(), secret))
            });
            let Some((service, secret)) = touched else {
                continue;
            };
            tracing::info!(target: TARGET, package = found.id.as_str(), service, why = "secret changed", secret, "mcp restarted");
            self.restart_services(&found.id, manifest).await;
        }
    }
}

#[cfg(test)]
mod tests;

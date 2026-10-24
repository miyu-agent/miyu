//! 扩展的搜索来源（施工 N-5，设计 36 第二节第 5 条）：核心反向调声明了 `[[search]]` 的扩展的 `search.query`。联网包不归这一层，
//! 回应原样交回，由 `miyu-core` 读成搜到的几条（`miyu_net::SearchPort`）。
//!
//! 来源是 `<包>/<编号>`：这个包的清单里要声明了它，程序要开着、登记过连接（`provide`，工具可以是空的）。它回 JSON-RPC 错误、
//! `data.reason` 是 `limited` 的算限额，别的算这一次没成。

use std::time::Duration;

use serde_json::Value;

use crate::Core;

/// 这一次没问成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// 它说限额了。
    Limited,
    /// 别的：没声明、没开、超时、出错，原因一句英文。
    Failed(String),
}

/// 问来源 `source`（`<包>/<编号>`），参数 `params`，等 `timeout`：交回清单里写的名字（照 `en`、`zh`、`ja` 挑）和回应。
///
/// # Errors
///
/// 见 [`Asked`]。
pub async fn query(
    core: &Core,
    source: &str,
    params: Value,
    timeout: Duration,
) -> Result<(String, Value), Asked> {
    let (package, id) = source
        .split_once('/')
        .ok_or_else(|| Asked::Failed("not an extension source".to_string()))?;
    let packages = core.system_packages();
    let declared = packages
        .iter()
        .filter(|found| found.id == package)
        .filter_map(|found| found.read.as_ref().ok())
        .find_map(|manifest| manifest.search.iter().find(|one| one.id == id))
        .ok_or_else(|| Asked::Failed(format!("{source} is not declared")))?;
    let name = ["en", "zh", "ja"]
        .iter()
        .find_map(|language| declared.name.get(*language).cloned())
        .unwrap_or_else(|| source.to_string());
    let peer = core
        .provided
        .peer(package)
        .ok_or_else(|| Asked::Failed(format!("{package} is not running")))?;
    match tokio::time::timeout(timeout, peer.call("search.query", params)).await {
        Err(_) => Err(Asked::Failed("no answer in time".to_string())),
        Ok(Err(_)) => Err(Asked::Failed(format!("{package} went away"))),
        Ok(Ok(Ok(result))) => Ok((name, result)),
        Ok(Ok(Err(error))) if error["data"]["reason"] == "limited" => Err(Asked::Limited),
        Ok(Ok(Err(error))) => Err(Asked::Failed(
            error["message"]
                .as_str()
                .unwrap_or("the extension answered with an error")
                .chars()
                .take(200)
                .collect(),
        )),
    }
}

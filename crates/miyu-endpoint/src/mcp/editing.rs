//! 头改个人设置里的 MCP 服务（施工 X-26，`docs/blueprint/mcp.md`「个人设置里的一张表」第 2 条：头要做编辑页的另给方法）：
//! `mcp.set` 加上或者整条换掉一张 `[mcp.<名字>]`，`mcp.remove` 删掉一张。只给本机的人用。
//!
//! 写之前照读服务表的规矩查这一张（[`personal_mcp`]：名字的写法、命令不空、给哪种会话、时限……），写法照 `config.set`
//! 的（`config/rewrite.rs`：带版本，不覆盖同时的手改）。写成了，正在跑的服务照「个人设置变了」那条路换（`home.rs` 的
//! `follow_home_mcp`）：新写的起，改了的停下再起，删掉的停下、包里被它盖住的起回来。
//!
//! 随包来的服务（个人设置里没写这个名字、有包带着它）不能经 `mcp.set` 改：`server_from_package`。

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_config::Layer;
use miyu_config::package::{Problem, personal_mcp};
use miyu_kernel::id::CommandId;
use miyu_store::human::Human;

use super::written::{self, Entry};
use crate::Core;
use crate::config::methods::words;
use crate::config::rewrite::{Unwritten, rewrite};
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `mcp.set` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    /// 服务名：表名。
    name: String,
    /// 程序。
    command: String,
    /// 程序后面的参数。
    #[serde(default)]
    args: Vec<String>,
    /// 环境变量：值是字，或者 `{"secret": …}`、`{"env": …}`。
    #[serde(default)]
    env: Option<Map<String, Value>>,
    /// 给哪种会话；不写的照读服务表时的默认。
    #[serde(default)]
    venues: Option<Vec<String>>,
    /// 一次调用等多久，毫秒；不写的照读服务表时的默认。
    #[serde(default)]
    timeout_ms: Option<i64>,
    /// 这个服务的工具要先加载；不写的不写进去，照读服务表时的默认。
    #[serde(default)]
    need_load: Option<bool>,
    /// 这个服务的工具脚本里能调（施工 M-2）；不写的照原来写的留着，原来没写的照读服务表时的默认（能）。
    #[serde(default)]
    clicall: Option<bool>,
}

/// `mcp.remove` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveParams {
    /// 服务名。
    name: String,
}

/// `mcp.set`：查过写法，在个人设置里加上或者整条换掉这一张，回应 `{"name"}`。
///
/// # Errors
///
/// 写错了的 `mcp_invalid`（`data.problem` 照连接的语言说哪里不对）；个人设置里没写、有包带着同名服务的
/// `server_from_package`；个人设置读不进来、`mcp` 写成了放不下的样子的 `config_file_broken`；别的同 `config.set`。
pub(crate) fn set(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: SetParams,
) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let name = params.name.clone();
    let entry = Entry {
        name: params.name,
        command: std::iter::once(params.command).chain(params.args).collect(),
        env: params.env.unwrap_or_default().into_iter().collect(),
        venues: params.venues,
        timeout_ms: params.timeout_ms,
        need_load: params.need_load,
        clicall: params.clicall,
    };
    let block = written::block(&entry).ok_or(Refusal::BAD_PARAMS)?;
    if let Some(problem) = personal_mcp(&block).1.first() {
        return Err(invalid(&words, problem));
    }
    let carried = !super::home::carrying(core, &name).is_empty();
    rewrite(core, peer, cause, Layer::Personal, |text| {
        match written::has(text, &name) {
            None => return Err(Unwritten::Blocked),
            Some(false) if carried => return Err(Refusal::SERVER_FROM_PACKAGE.into()),
            Some(_) => {}
        }
        let block = match (entry.clicall, written::kept_clicall(text, &name)) {
            (None, Some(kept)) => written::block(&Entry {
                clicall: Some(kept),
                ..entry.clone()
            })
            .ok_or(Unwritten::Blocked)?,
            _ => block.clone(),
        };
        written::put(text, &name, &block)
            .map(Some)
            .ok_or(Unwritten::Blocked)
    })?;
    Ok(json!({ "name": name }))
}

/// `mcp.remove`：从个人设置里删掉这一张，回应 `{}`。
///
/// # Errors
///
/// 个人设置里没写这个名字的 `unknown_server`；个人设置读不进来、`mcp` 写成了放不下的样子的 `config_file_broken`；别的同
/// `config.set`。
pub(crate) fn remove(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: RemoveParams,
) -> Result<Value, Refusal> {
    let name = params.name;
    rewrite(
        core,
        peer,
        cause,
        Layer::Personal,
        |text| match written::has(text, &name) {
            None => Err(Unwritten::Blocked),
            Some(false) => Err(Refusal::UNKNOWN_SERVER.into()),
            Some(true) => written::remove(text, &name)
                .map(Some)
                .ok_or(Unwritten::Blocked),
        },
    )?;
    Ok(json!({}))
}

/// 写错了的一张：照连接的语言说一句（`package-problems/<code>`，同装包时清单写错的说法），说不出的照英文原话。
fn invalid(words: &Human, problem: &Problem) -> Refusal {
    let said = crate::packages::sentence(words, problem.code.as_str(), &problem.detail)
        .unwrap_or_else(|| problem.message.clone());
    Refusal::mcp_invalid(said)
}

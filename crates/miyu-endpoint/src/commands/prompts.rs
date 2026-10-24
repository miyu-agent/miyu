//! MCP 服务的提示词当斜杠命令（施工 X-39 之二，`docs/blueprint/slash.md`「是什么」第三种，`mcp.md`「怎么走」第六条）：名字是
//! `<服务名>:<提示词名>`（`mcp/prompts.rs` 清理好的），菜单里排在模板后面；`command.run` 照它找服务、判身份和预设（跟着服务的
//! 功能 `mcp-<服务名>`，不另设功能）、对上参数、`prompts/get`，取回来的并成这个人说的一句发出去，之后同模板命令。只在本机的会话
//! 里认：MCP 服务默认只给本机，通讯平台那边也没有菜单。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_config::package::Access;
use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::event::TypedCommand;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::session::Command;
use miyu_mcp::{Argument, Client};
use miyu_policy::slash::{next_word, rest_word};
use miyu_store::blob::Blobs;
use miyu_tool::Picture;

use super::{RunParams, accepted, access, command, passes, speaker, templates, turned_off};
use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::mcp::prompts::Offered;
use crate::refusal::Refusal;

/// 这一刻用得上的一个提示词：在跑的服务的。
pub(super) struct Usable {
    /// 服务的包（个人设置里的是 `~home`）：预设照它开关。
    pub(super) package: String,
    /// 服务名。
    pub(super) service: String,
    client: Arc<Client>,
    /// 登记的那一个。
    pub(super) offered: Offered,
}

impl Usable {
    /// 它跟着的功能：服务的 `mcp-<服务名>`。
    pub(super) fn feature(&self) -> String {
        format!("mcp-{}", self.service)
    }
}

/// 这一刻在跑、列过了的服务登记的提示词，照命令名排。
pub(super) fn usable(core: &Core) -> Vec<Usable> {
    let mut usable: Vec<Usable> = core
        .mcp
        .prompts()
        .into_iter()
        .flat_map(|prompts| {
            let (package, service, client) = (prompts.package, prompts.service, prompts.client);
            prompts.offered.into_iter().map(move |offered| Usable {
                package: package.clone(),
                service: service.clone(),
                client: Arc::clone(&client),
                offered,
            })
        })
        .collect();
    usable.sort_by(|a, b| a.offered.command.cmp(&b.offered.command));
    usable
}

/// `command.catalog` 的一项：说明照服务写的 `title`，没有的照 `description`（服务的字不分语言）；名字后面跟什么照声明的参数拼，
/// 必填的 `<名字>`、可选的 `[名字]`。
pub(super) fn listed(offered: &Offered) -> Value {
    let prompt = &offered.prompt;
    let summary = prompt
        .title
        .clone()
        .or_else(|| prompt.description.clone())
        .unwrap_or_default();
    let mut item = json!({"name": offered.command, "aliases": [], "summary": summary});
    let argument: Vec<String> = prompt
        .arguments
        .iter()
        .map(|argument| match argument.required {
            true => format!("<{}>", argument.name),
            false => format!("[{}]", argument.name),
        })
        .collect();
    if !argument.is_empty() {
        item["argument"] = json!(argument.join(" "));
    }
    item
}

/// 对上的参数（名字到字），和没声明参数时接在末尾的字。
pub(super) type Matched = (BTreeMap<String, String>, Option<String>);

/// 人跟的字 `rest` 照声明的参数 `declared` 的先后对上：照切词（[`miyu_policy::slash::next_word`]，带引号的是一个词，施工 X-40）
/// 一个一个对，最后一个参数拿剩下的整段（只有一个参数的拿全部；正好是一个带引号的词的去掉引号）。空的值（`""`）当没给。没声明
/// 参数的交回空的，字照模板的规矩接在末尾（交回的第二格）。必填的没给的交回缺哪几个。
pub(super) fn arguments(declared: &[Argument], rest: &str) -> Result<Matched, Vec<String>> {
    let mut left = rest.trim();
    let mut given = BTreeMap::new();
    if declared.is_empty() {
        return Ok((given, (!left.is_empty()).then(|| left.to_string())));
    }
    for (n, argument) in declared.iter().enumerate() {
        if left.is_empty() {
            break;
        }
        let value = match (n + 1 == declared.len(), next_word(left)) {
            (false, Some((word, more))) => {
                left = more;
                word
            }
            _ => rest_word(std::mem::take(&mut left)),
        };
        if !value.is_empty() {
            given.insert(argument.name.clone(), value.to_string());
        }
    }
    let missing: Vec<String> = declared
        .iter()
        .filter(|argument| argument.required && !given.contains_key(&argument.name))
        .map(|argument| argument.name.clone())
        .collect();
    match missing.is_empty() {
        true => Ok((given, None)),
        false => Err(missing),
    }
}

/// `command.run` 打的是 `<服务名>:<提示词名>`：找会话、认说话的人、找提示词（服务配着没在跑、没列出这一个的
/// `command_unavailable`，没有这个服务的 `unknown_command`），判身份、预设，对上参数（缺必填的 `command_failed`，说缺哪几个），
/// 照服务的 `timeout_ms` 取、最多一分钟（到时没答的 `command_unavailable`；回错、要补信息的 `command_failed`，`data.why` 照原话），
/// 并成一句发出去，交回 `{"command", "events", "said": ""}`。
pub(super) async fn run(
    core: &Arc<Core>,
    peer: &Peer,
    id: &CommandId,
    params: &RunParams,
    (name, rest): (&str, &str),
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session).await?;
    let handle = found.handle.clone();
    let by = speaker(
        core,
        &handle,
        params.person.as_ref(),
        params.as_external.clone(),
    )?;
    let Some(entry) = usable(core)
        .into_iter()
        .find(|entry| entry.offered.command == name)
    else {
        let service = name.split_once(':').map_or(name, |(service, _)| service);
        return Err(match configured(core, service) {
            true => Refusal::COMMAND_UNAVAILABLE,
            false => Refusal::UNKNOWN_COMMAND,
        });
    };
    if handle.venue().as_str() != LOCAL {
        return Err(Refusal::COMMAND_UNAVAILABLE);
    }
    match access(core, name, None) {
        Access::Off => return Err(turned_off(core, peer, name).await),
        granted if !passes(granted, &by) => return Err(Refusal::COMMAND_NOT_ALLOWED),
        _ => {}
    }
    let feature = entry.feature();
    templates::opened_by(core, peer, &handle, (name, &feature, &entry.package)).await?;
    let prompt = &entry.offered.prompt;
    let (arguments, tail) = match arguments(&prompt.arguments, rest) {
        Ok(matched) => matched,
        Err(missing) => {
            let why = super::words(core, peer, &super::why::missing_arguments(missing)).await;
            return Err(Refusal::COMMAND_FAILED.because(why));
        }
    };
    let asked = tokio::time::timeout(
        entry.offered.wait,
        entry.client.prompt(&prompt.name, &arguments),
    );
    let messages = match asked.await {
        Ok(Ok(messages)) => messages,
        Ok(Err(error)) => return Err(Refusal::COMMAND_FAILED.because(error.to_string())),
        // 到时没答同扩展的命令（施工 X-39 补）。
        Err(_) => return Err(Refusal::COMMAND_UNAVAILABLE),
    };
    let (mut blocks, pictures) =
        crate::mcp::prompts::said(core, messages).ok_or(Refusal::INTERNAL)?;
    if let Some(tail) = tail {
        blocks.push(Block::Text(Text { text: tail }));
    }
    let blobs = Blobs::new(core.root.blobs(handle.owner()));
    let images = tokio::task::spawn_blocking(move || stored(&blobs, pictures))
        .await
        .map_err(|_| Refusal::INTERNAL)??;
    blocks.extend(images);
    let typed = TypedCommand {
        name: name.to_string(),
        text: params.text.trim().to_string(),
    };
    let sent = Command::Typed {
        blocks,
        command: typed,
    };
    let events = accepted(command(core, &session, &handle, id, &by, sent).await?)?;
    Ok(json!({"command": name, "events": events, "said": ""}))
}

/// 有这个服务：个人设置里写了的、装了的包带的（没在跑的也算）。
fn configured(core: &Core, service: &str) -> bool {
    core.home_mcp
        .now()
        .iter()
        .any(|server| server.name == service)
        || core.system_packages().iter().any(|found| {
            found
                .read
                .as_ref()
                .is_ok_and(|manifest| manifest.mcp.iter().any(|server| server.name == service))
        })
}

/// 取回来的图存成会话属主的 blob、换成图片块。有一张存不下来的照内部出错拒：少了字节的图以后每次请求都发不出去。碰磁盘。
fn stored(blobs: &Blobs, pictures: Vec<Picture>) -> Result<Vec<Block>, Refusal> {
    pictures
        .into_iter()
        .map(|picture| {
            let blob = blobs.put(&picture.bytes).map_err(|_| Refusal::INTERNAL)?;
            Ok(Block::Image(Image {
                blob,
                name: None,
                media_type: picture.media_type,
                width: picture.width,
                height: picture.height,
                path: None,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests;

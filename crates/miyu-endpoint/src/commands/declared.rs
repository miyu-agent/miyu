//! 扩展声明的斜杠命令（施工 X-14 下，`docs/blueprint/slash.md`「扩展的命令」）：清单 `[[slash]]` 写的，人敲了核心反向调用那个
//! 扩展的 `command.call {command, argument, text, session, by, owner, language}`，扩展答 `{"said"}`（一段文字，可以多行），核心记
//! 一条 `command.ran`、把它当回执交回去。扩展没连上、断了、到时没答的 `command_unavailable`；答了错、写法不对的
//! `command_failed`，`data.why` 是它的原话。和核心登记的撞名的不算（内置的赢）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_config::package::SlashCommand;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::Command;
use miyu_session::Handle;

use super::why::no_reply;
use super::{ANSWER_WAIT, RunParams, accepted, command, words};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 一个扩展声明的一条命令。
#[derive(Debug, Clone)]
pub(crate) struct Declared {
    /// 声明它的包。
    pub(crate) package: String,
    /// 清单里写的那一段。
    pub(crate) command: SlashCommand,
}

/// 装着的扩展声明的全部命令（清单读成了的），照名字排；和登记表撞名的不算。只看核心手里的清单，不碰磁盘。
pub(crate) fn found(core: &Core) -> Vec<Declared> {
    let mut all: Vec<Declared> = core
        .packages()
        .iter()
        .filter_map(|one| Some((one.id.clone(), one.read.as_ref().ok()?)))
        .flat_map(|(package, manifest)| {
            manifest.slash.iter().map(move |command| Declared {
                package: package.clone(),
                command: command.clone(),
            })
        })
        .filter(|declared| core.commands.find(&declared.command.name).is_none())
        .collect();
    all.sort_by(|a, b| a.command.name.cmp(&b.command.name));
    all
}

/// 名字是 `name` 的那一条；没有的没有。
pub(crate) fn named(core: &Core, name: &str) -> Option<Declared> {
    found(core)
        .into_iter()
        .find(|declared| declared.command.name == name)
}

/// 交给扩展办：反向调用它的 `command.call`，等它答一段话；记一条 `command.ran`，交回 `{"command", "events", "said"}`。
pub(super) async fn run(
    core: &Arc<Core>,
    peer: &Peer,
    id: &CommandId,
    params: &RunParams,
    (session, handle, by): (&SessionId, &Handle, &By),
    (declared, rest): (&Declared, &str),
) -> Result<Value, Refusal> {
    let name = declared.command.name.as_str();
    let noted = CommandId::parse(&format!("{id}/ran")).map_err(|_| Refusal::BAD_PARAMS)?;
    let connection = core
        .provided
        .peer(&declared.package)
        .ok_or(Refusal::COMMAND_UNAVAILABLE)?;
    let asked = json!({
        "command": name,
        "argument": rest,
        "text": params.text.trim(),
        "session": session.as_str(),
        "by": by,
        "owner": handle.owner().as_str(),
        "language": peer.language,
    });
    let answer = tokio::time::timeout(ANSWER_WAIT, connection.call("command.call", asked))
        .await
        .map_err(|_| Refusal::COMMAND_UNAVAILABLE)?
        .map_err(|_| Refusal::COMMAND_UNAVAILABLE)?;
    let said = match answer {
        Ok(result) => match result.get("said").and_then(Value::as_str) {
            Some(said) => said.to_string(),
            None => {
                let why = words(core, peer, &no_reply(name)).await;
                return Err(Refusal::COMMAND_FAILED.because(why));
            }
        },
        Err(error) => {
            let why = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default();
            return Err(Refusal::COMMAND_FAILED.because(why.to_string()));
        }
    };
    let note = Command::Ran {
        text: params.text.clone(),
        command: name.to_string(),
    };
    let events = accepted(command(core, session, handle, &noted, by, note).await?)?;
    Ok(json!({"command": name, "events": events, "said": said}))
}

/// `command.catalog` 里的一项：名字、没有别名、说明照连接的语言挑（没写这种语言的照 `en`、`zh`、`ja`），写了的提示。
pub(super) fn listed(declared: &Declared, language: &str) -> Value {
    let summary = crate::personas::pick(&declared.command.summary, language).unwrap_or_default();
    let mut item = json!({"name": declared.command.name, "aliases": [], "summary": summary});
    if let Some(argument) = declared
        .command
        .argument
        .as_ref()
        .and_then(|argument| crate::personas::pick(argument, language))
    {
        item["argument"] = json!(argument);
    }
    item
}

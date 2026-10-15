//! 斜杠命令模板（施工 X-14 上，`docs/blueprint/slash.md`）：登记表里没有的名字照这个会话属主的家目录、装了的包找
//! `commands/<名字>.md`，展开成一段话当这个人说的一句发出去（`message.user` 带上人敲的那一行），时间线上有那一行，不另回执、
//! 不另记 `command.ran`。和核心登记的撞名的不算（内置的赢）。谁能用：先过内置命令那一关，再看会话的预设开没开这条命令
//! （每条一个功能 `command-<名字>`）。
//!
//! 每次照这一刻找：文件少、只读开头，改了模板不用重启。碰磁盘的放在阻塞线程里。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::TypedCommand;
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::session::Command;
use miyu_policy::slash::{expand, feature_id};
use miyu_session::Handle;
use miyu_store::scripts::HOME_PACKAGE;
use miyu_store::slash::{self, Command as Template};

use super::{RunParams, Said, accepted, command, may_run, speaker, why, words};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 账号 `owner` 用得上的模板命令：家目录的、装了的包的，同名的先找到的赢，和登记表撞名的不算。照名字排。
pub(crate) async fn found(core: &Core, owner: &AccountId) -> Vec<Template> {
    let (packages, root, owner) = (core.packages(), core.root.clone(), owner.clone());
    let found = tokio::task::spawn_blocking(move || slash::find(&packages, &root, &owner).0)
        .await
        .unwrap_or_default();
    found
        .into_iter()
        .filter(|template| core.commands.find(&template.name).is_none())
        .collect()
}

/// 模板命令挂在哪个包下（预设照它开关）：包带的照包的编号，自己放的是 [`HOME_PACKAGE`]。
fn package(template: &Template) -> &str {
    template.package.as_deref().unwrap_or(HOME_PACKAGE)
}

/// 会话的预设开没开 `template`：开着的、没有预设的、说不准的（会话表里查不到、预设读不出）是 `Ok`；关着的拒
/// `command_not_allowed`，`data.why` 带上预设的名字。
pub(super) async fn opened(
    core: &Core,
    peer: &Peer,
    handle: &Handle,
    template: &Template,
) -> Result<(), Refusal> {
    let Some(preset) = why::listed(core, handle)
        .await
        .and_then(|listed| listed.preset)
    else {
        return Ok(());
    };
    let Ok(found) = crate::presets::resolve(core, Some(&preset)).await else {
        return Ok(());
    };
    if found
        .file
        .opens_in(&feature_id(&template.name), package(template))
    {
        return Ok(());
    }
    let name = crate::personas::label(found.file.name.as_ref(), peer.language).unwrap_or(preset);
    let said = Said {
        key: "commands/unavailable/command-preset-off",
        fields: vec![("preset", name), ("command", template.name.clone())],
    };
    Err(Refusal::COMMAND_NOT_ALLOWED.because(words(core, peer, &said).await))
}

/// `command.run` 打的是模板命令 `name`、后面跟 `rest`：找会话、认说话的人、找模板、判身份、看预设，展开发出去，交回
/// `{"command", "events", "said": ""}`。找不到的 `unknown_command`；展开完是空的照内核拒 `empty_message`。
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
    let by = speaker(core, &handle, params.as_external.clone())?;
    // 先认再判身份：认不出的照旧 `unknown_command`，场所里的桥照它把话当成平常说的记下。
    let template = found_named(core, handle.owner(), name)
        .await
        .ok_or(Refusal::UNKNOWN_COMMAND)?;
    if !may_run(&by) {
        return Err(Refusal::COMMAND_NOT_ALLOWED);
    }
    opened(core, peer, &handle, &template).await?;
    let expanded = expand(&template.template.body, rest);
    let blocks = match expanded.is_empty() {
        true => Vec::new(),
        false => vec![Block::Text(Text { text: expanded })],
    };
    let typed = TypedCommand {
        name: template.name.clone(),
        text: params.text.trim().to_string(),
    };
    let sent = Command::Typed {
        blocks,
        command: typed,
    };
    let events = accepted(command(core, &session, &handle, id, &by, sent).await?)?;
    Ok(json!({"command": template.name, "events": events, "said": ""}))
}

/// 名字是 `name` 的那一条模板命令；没有的没有。
async fn found_named(core: &Core, owner: &AccountId, name: &str) -> Option<Template> {
    found(core, owner)
        .await
        .into_iter()
        .find(|template| template.name == name)
}

/// `command.catalog` 里的一项：名字、没有别名、说明（照连接的语言挑，没写的照正文第一行）、写了的名字后面跟什么的提示。
pub(super) fn listed(template: &Template, language: &str) -> Value {
    let summary =
        crate::personas::pick(&template.template.summary, language).unwrap_or_else(|| {
            template
                .template
                .body
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default()
                .to_string()
        });
    let mut item = json!({"name": template.name, "aliases": [], "summary": summary});
    if let Some(argument) = &template.template.argument {
        item["argument"] = json!(argument);
    }
    item
}

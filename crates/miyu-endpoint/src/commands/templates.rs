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

use super::{
    RunParams, Said, accepted, access, command, declared, pages, passes, speaker, turned_off, why,
    words,
};
use miyu_config::package::Access;

use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::refusal::Refusal;

/// 账号 `owner` 用得上的模板命令：家目录的、装了的包的，同名的先找到的赢，和登记表撞名的不算。照名字排。
pub(crate) async fn found(core: &Core, owner: &AccountId) -> Vec<Template> {
    let (packages, root, owner) = (core.packages_of(owner), core.root.clone(), owner.clone());
    let found = tokio::task::spawn_blocking(move || slash::find(&packages, &root, &owner).0)
        .await
        .unwrap_or_default();
    found
        .into_iter()
        .filter(|template| core.commands.find_present(core, &template.name).is_none())
        .collect()
}

/// 模板命令挂在哪个包下（预设照它开关）：包带的照包的编号，自己放的是 [`HOME_PACKAGE`]。
pub(super) fn package(template: &Template) -> &str {
    template.package.as_deref().unwrap_or(HOME_PACKAGE)
}

/// 会话的预设开没开命令 `name`（挂在包 `package` 下，功能 `command-<名字>`；模板、扩展声明的都照它）：开着的、没有预设的、
/// 说不准的（会话表里查不到、预设读不出）是 `Ok`；关着的拒 `command_not_allowed`，`data.why` 带上预设的名字。
pub(super) async fn opened(
    core: &Core,
    peer: &Peer,
    handle: &Handle,
    (name, package): (&str, &str),
) -> Result<(), Refusal> {
    opened_by(core, peer, handle, (name, &feature_id(name), package)).await
}

/// 同 [`opened`]，照功能 `feature` 判（施工 X-39：MCP 的提示词跟着服务的 `mcp-<服务名>` 走），拒的时候说命令 `name`。
pub(super) async fn opened_by(
    core: &Core,
    peer: &Peer,
    handle: &Handle,
    (name, feature, package): (&str, &str, &str),
) -> Result<(), Refusal> {
    let Some(preset) = why::listed(core, handle)
        .await
        .and_then(|listed| listed.preset)
    else {
        return Ok(());
    };
    // 照会话的属主找（施工 Y-2 中上）。
    let Ok(found) = crate::presets::resolve(core, handle.owner(), Some(&preset)).await else {
        return Ok(());
    };
    if found.file.opens_in(feature, package) {
        return Ok(());
    }
    let label = crate::personas::label(found.file.name.as_ref(), peer.language).unwrap_or(preset);
    let said = Said {
        key: "commands/unavailable/command-preset-off",
        fields: vec![("preset", label), ("command", name.to_string())],
        lists: Vec::new(),
    };
    Err(Refusal::COMMAND_NOT_ALLOWED.because(words(core, peer, &said).await))
}

/// `command.run` 打的不是登记表里的 `name`、后面跟 `rest`：找会话、认说话的人，再认是哪一种（自己放的模板先，再扩展声明的
/// （施工 X-14 下，交给 [`declared::run`]），再包里的模板；包里的两种装包时查过、撞不上），判身份、看预设。模板展开发出去，交回
/// `{"command", "events", "said": ""}`。都没有的 `unknown_command`；展开完是空的照内核拒 `empty_message`。
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
    // 先认再判身份：认不出的照旧 `unknown_command`，场所里的桥照它把话当成平常说的记下。
    let template = found_named(core, handle.owner(), name).await;
    let own = matches!(&template, Some(own) if own.package.is_none());
    // 会话属主关了的包（施工 U23 末下）：它声明的命令、整页当没有。
    let off = core.packages_off(handle.owner());
    // 整页的命令（施工 X-15 下）：只在本机的会话里认，自己放的模板盖住它；只叫头打开那一页，不碰会话。
    if !own
        && handle.venue().as_str() == LOCAL
        && let Some(page) = pages::named(core, name).filter(|page| !off.contains(page.package()))
    {
        // 停用了的不开（施工 O-6 六补）；整页只在本机开，身份那一关本来就是本人。
        if access(core, name, None) == Access::Off {
            return Err(turned_off(core, peer, name).await);
        }
        return Ok(pages::opened(&page));
    }
    let declared = match &template {
        Some(own) if own.package.is_none() => None,
        _ => declared::named(core, name).filter(|one| !off.contains(&one.package)),
    };
    if template.is_none() && declared.is_none() {
        return Err(Refusal::UNKNOWN_COMMAND);
    }
    // 谁能用（施工 O-6 六补）：扩展的照清单写的出厂值，模板没有出厂值。
    let shipped = declared
        .as_ref()
        .and_then(|declared| declared.command.access);
    match access(core, name, shipped) {
        Access::Off => return Err(turned_off(core, peer, name).await),
        granted if !passes(granted, &by) => return Err(Refusal::COMMAND_NOT_ALLOWED),
        _ => {}
    }
    if let Some(declared) = declared {
        opened(
            core,
            peer,
            &handle,
            (&declared.command.name, &declared.package),
        )
        .await?;
        return declared::run(
            core,
            peer,
            id,
            params,
            (&session, &handle, &by),
            (&declared, rest),
        )
        .await;
    }
    let template = template.ok_or(Refusal::UNKNOWN_COMMAND)?;
    opened(core, peer, &handle, (&template.name, package(&template))).await?;
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

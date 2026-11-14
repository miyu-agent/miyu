//! `command.catalog`（施工 O-6 补，`docs/blueprint/protocol.md`「`command.catalog`」）：核心认的斜杠命令列给头的命令菜单、
//! `/help`。不写会话的列全部；写了的只列这个连接在这个会话里打了不会被拒的，和 `command.run` 同一份判法（[`super::allowed`]）。
//! 只收本机的会话：场所会话的命令由桥代表外部的人说，桥自己知道。列表不推送，头换会话、打开菜单时问。
//!
//! 模板命令（施工 X-14 上，[`super::templates`]）、扩展声明的命令（施工 X-14 下，[`super::declared`]）、MCP 服务的提示词（施工
//! X-39，[`super::prompts`]）、整页的命令（施工 X-15 下，[`super::pages`]，带 `opens`）一起列，照名字排进去；能用 `$` 点名的
//! 技能（[`super::skills`]，施工 SN-1）另列在 `skills`：
//! 不写会话的照数据根、装了的包找全部，跟着包的核心命令照默认预设筛（施工 CC-1）；写了的只列会话的预设开着的。
//! 自己放的模板盖住扩展声明的同名的。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::Access;
use miyu_kernel::id::SessionId;
use miyu_session::Handle;
use miyu_store::slash::Command as Template;

use super::declared::{self, Declared};
use super::{Said, Slash, access, allowed, pages, prompts, skills, templates, usage, words};
use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::refusal::Refusal;

/// 照会话挑出来的：核心登记的、模板、扩展声明的，和会话（不写会话的没有，MCP 的提示词照它看预设，施工 X-39）。
type Found<'a> = (
    Vec<&'a Arc<dyn Slash>>,
    Vec<Template>,
    Vec<Declared>,
    Option<Handle>,
);

/// `command.catalog` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogParams {
    #[serde(default)]
    session: Option<String>,
}

/// `command.catalog`：交回 `{"commands": [{name, aliases, summary, argument?}], "skills": [{name, summary}]}`，照名字排；能用
/// `$` 点名的技能另列在 `skills`（施工 SN-1，原来混在命令里带 `"skill": true`）。
pub(crate) async fn catalog(
    core: &Arc<Core>,
    peer: &Peer,
    params: CatalogParams,
) -> Result<Value, Refusal> {
    // 包没装的命令不列（施工 BS-1 下下）。
    let all = core
        .commands
        .all()
        .filter(|slash| super::registry::present(core, slash.as_ref()));
    let declared = declared::found(core);
    let (listed, templates, declared, handle): Found<'_> = match params.session {
        // 不写会话的（施工 CC-1）：跟着包的命令照替谁干活的人的默认预设筛，开会话的话那个包没开的不列（`/loop`、`/goal` 在
        // 会话里照工具面上有没有那件工具认，没开的敲了是 `unknown_command`）。
        None => {
            let default = crate::presets::default_of(core);
            let listed = all
                .filter(|slash| {
                    slash.package().is_none_or(|package| {
                        crate::presets::package_on(core, Some(&default), package)
                    })
                })
                .collect();
            (listed, templates::found(core).await, declared, None)
        }
        Some(text) => {
            let session = SessionId::parse(&text).map_err(|_| Refusal::BAD_PARAMS)?;
            let found = core.sessions.get(core, &session).await?;
            let handle = &found.handle;
            if handle.venue().as_str() != LOCAL {
                return Err(Refusal::VENUE_SESSION);
            }
            let by = crate::sessions::person();
            let listed = all
                .filter(|slash| allowed(core, slash.as_ref(), handle, &by).is_ok())
                .collect();
            let mut opened = Vec::new();
            for template in templates::found(core).await {
                let key = (template.name.as_str(), templates::package(&template));
                if templates::opened(core, peer, handle, key).await.is_ok() {
                    opened.push(template);
                }
            }
            let mut usable = Vec::new();
            for one in declared {
                let key = (one.command.name.as_str(), one.package.as_str());
                if templates::opened(core, peer, handle, key).await.is_ok() {
                    usable.push(one);
                }
            }
            (listed, opened, usable, Some(handle.clone()))
        }
    };
    // 关了的包声明的命令、整页、提示词都不列（施工 U23 末下）。
    let off = core.packages_off();
    let declared: Vec<Declared> = declared
        .into_iter()
        .filter(|one| !off.contains(&one.package))
        .collect();
    let mut commands = Vec::with_capacity(listed.len() + templates.len());
    for slash in listed {
        let summary = words(core, peer, &Said::plain(slash.summary())).await;
        let mut item =
            json!({"name": slash.name(), "aliases": slash.aliases(), "summary": summary});
        let argument = match slash.argument() {
            Some(argument) => Some(words(core, peer, &Said::plain(argument)).await),
            None => None,
        };
        if let Some(argument) = &argument {
            item["argument"] = json!(argument);
        }
        // 几种写法（施工 O-6 四补）：参数写错的拒绝带的用法同一份（施工 LO-1，[`usage::forms`]）。
        let usage: Vec<Value> = usage::forms(core, peer, slash.as_ref())
            .await
            .into_iter()
            .map(|(form, summary)| json!({"form": form, "summary": summary}))
            .collect();
        if !usage.is_empty() {
            item["usage"] = json!(usage);
        }
        commands.push(item);
    }
    commands.extend(
        templates
            .iter()
            .map(|template| templates::listed(template, peer.language)),
    );
    commands.extend(
        declared
            .iter()
            .filter(|one| !templates.iter().any(|t| t.name == one.command.name))
            .map(|one| declared::listed(one, peer.language)),
    );
    // MCP 服务的提示词（施工 X-39）：在跑的服务登记的、这个人用得上的（施工 U23 中上），排在模板后面；写了会话的只列预设开着
    // 这个服务的。
    for entry in prompts::usable(core) {
        let opened = match &handle {
            None => true,
            Some(handle) => {
                let key = (
                    entry.offered.command.as_str(),
                    entry.feature(),
                    entry.package.as_str(),
                );
                templates::opened_by(core, peer, handle, (key.0, &key.1, key.2))
                    .await
                    .is_ok()
            }
        };
        if opened {
            commands.push(prompts::listed(&entry.offered));
        }
    }
    // 整页的命令（施工 X-15 下）：不归预设管；自己放的同名模板盖住它。
    commands.extend(
        pages::found(core)
            .iter()
            .filter(|page| !off.contains(page.package()))
            .filter(|page| {
                !templates
                    .iter()
                    .any(|t| t.package.is_none() && t.name == page.name)
            })
            .map(|page| pages::listed(page, peer.language)),
    );
    // 能点名用的技能（施工 PT-2 下；SN-1 起另列在 `skills`，头照它做 `$` 菜单，和命令不再撞名）：写了会话的只列预设开着的。
    let mut skills_listed = Vec::new();
    for skill in skills::found(core).await {
        if let Some(handle) = &handle
            && skills::opened(core, peer, handle, &skill).await.is_err()
        {
            continue;
        }
        skills_listed.push(skills::listed(&skill, peer.language));
    }
    // 停用了的不列（施工 O-6 六补）：扩展的照清单写的出厂值算。
    let shipped = |name: &str| {
        declared
            .iter()
            .find(|one| one.command.name == name)
            .and_then(|one| one.command.access)
    };
    commands.retain(|item| {
        let name = item["name"].as_str().unwrap_or_default();
        access(core, name, shipped(name)) != Access::Off
    });
    commands.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(json!({ "commands": commands, "skills": skills_listed }))
}

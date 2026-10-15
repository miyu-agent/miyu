//! `command.catalog`（施工 O-6 补，`docs/blueprint/protocol.md`「`command.catalog`」）：核心认的斜杠命令列给头的命令菜单、
//! `/help`。不写会话的列全部；写了的只列这个连接在这个会话里打了不会被拒的，和 `command.run` 同一份判法（[`super::allowed`]）。
//! 只收本机的会话：场所会话的命令由桥代表外部的人说，桥自己知道。列表不推送，头换会话、打开菜单时问。
//!
//! 模板命令（施工 X-14 上，[`super::templates`]）、扩展声明的命令（施工 X-14 下，[`super::declared`]）、整页的命令（施工 X-15 下，
//! [`super::pages`]，带 `opens`）一起列，照名字排进去：
//! 不写会话的照管理员的家目录、装了的包找全部；写了的照会话属主找，只列预设开着的。自己放的模板盖住扩展声明的同名的。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_store::slash::Command as Template;

use super::declared::{self, Declared};
use super::{Said, Slash, allowed, pages, templates, words};
use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::refusal::Refusal;
use crate::sessions::admin;

/// `command.catalog` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogParams {
    #[serde(default)]
    session: Option<String>,
}

/// `command.catalog`：交回 `{"commands": [{name, aliases, summary, argument?}]}`，照名字排。
pub(crate) async fn catalog(
    core: &Arc<Core>,
    peer: &Peer,
    params: CatalogParams,
) -> Result<Value, Refusal> {
    let all = core.commands.all();
    let declared = declared::found(core);
    let (listed, templates, declared): (Vec<&Arc<dyn Slash>>, Vec<Template>, Vec<Declared>) =
        match params.session {
            None => (
                all.collect(),
                templates::found(core, &core.admin).await,
                declared,
            ),
            Some(text) => {
                let session = SessionId::parse(&text).map_err(|_| Refusal::BAD_PARAMS)?;
                let found = core.sessions.get(core, &session).await?;
                let handle = &found.handle;
                if handle.venue().as_str() != LOCAL {
                    return Err(Refusal::VENUE_SESSION);
                }
                let by = admin(core);
                let listed = all
                    .filter(|slash| allowed(core, slash.as_ref(), handle, &by).is_ok())
                    .collect();
                let mut opened = Vec::new();
                for template in templates::found(core, handle.owner()).await {
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
                (listed, opened, usable)
            }
        };
    let mut commands = Vec::with_capacity(listed.len() + templates.len());
    for slash in listed {
        let summary = words(core, peer, &Said::plain(slash.summary())).await;
        let mut item =
            json!({"name": slash.name(), "aliases": slash.aliases(), "summary": summary});
        if let Some(argument) = slash.argument() {
            item["argument"] = json!(words(core, peer, &Said::plain(argument)).await);
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
    // 整页的命令（施工 X-15 下）：不归预设管；自己放的同名模板盖住它。
    commands.extend(
        pages::found(core)
            .iter()
            .filter(|page| {
                !templates
                    .iter()
                    .any(|t| t.package.is_none() && t.name == page.name)
            })
            .map(|page| pages::listed(page, peer.language)),
    );
    commands.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(json!({ "commands": commands }))
}

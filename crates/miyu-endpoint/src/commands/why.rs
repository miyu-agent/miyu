//! 用不了的命令为什么用不了（施工 O-6 再补，2026-10-09 项目主人在网页上验收：带了人格、预设没开记忆的会话打 `/remember`，
//! 不知道为什么）：`command.run` 拒绝的 `data.why`，照连接的语言。记忆用不了的说具体：人格记忆没装（施工 R-10）；会话没有
//! 人格；会话的预设没开记忆，带上预设的名字，人才知道去哪改。长期目标用不了的（施工 X-8 下）同样说具体：没装、预设没开；
//! 群里、私聊、子代理照拒绝的那一句。别的照拒绝的那一句。

use miyu_policy::preset::MEMORY;
use miyu_session::Handle;

use super::{Said, words};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 长期目标这个包、功能的编号（同 `miyu_goal::PACKAGE`；端点不依赖软件包的 crate）。
pub(super) const GOAL_PACKAGE: &str = "goal";

/// `refused` 这一句拒绝，说成给人看的为什么。
pub(super) async fn why(core: &Core, peer: &Peer, handle: &Handle, refused: &Refusal) -> String {
    let said = match refused.reason {
        reason if reason == Refusal::MEMORY_UNAVAILABLE.reason => memory(core, peer, handle).await,
        reason if reason == Refusal::GOAL_UNAVAILABLE.reason => goal(core, peer, handle).await,
        _ => None,
    };
    if let Some(said) = said {
        let text = words(core, peer, &said).await;
        if !text.is_empty() {
            return text;
        }
    }
    refused.message(peer.locale).to_string()
}

/// 长期目标为什么用不了（施工 X-8 下）：没装的、本机的主会话预设没开的；别的（群里、私聊、子代理，说不准的）是 `None`。
async fn goal(core: &Core, peer: &Peer, handle: &Handle) -> Option<Said> {
    if !crate::packages::is_installed(&core.packages(), GOAL_PACKAGE) {
        return Some(Said::plain("commands/unavailable/goal-not-installed"));
    }
    let listed = listed(core, handle).await?;
    if handle.venue().as_str() != crate::list::LOCAL || listed.parent.is_some() {
        return None;
    }
    preset_off(
        core,
        peer,
        listed.preset?,
        GOAL_PACKAGE,
        "commands/unavailable/goal-preset-off",
    )
    .await
}

/// 会话表里的这一条；查不到的没有。
pub(super) async fn listed(core: &Core, handle: &Handle) -> Option<crate::list::Listed> {
    let (root, admin, index) = (core.root.clone(), core.admin.clone(), core.index.clone());
    let id = handle.id().clone();
    tokio::task::spawn_blocking(move || crate::list::one(&root, &admin, &index, &id, false))
        .await
        .ok()?
}

/// 会话的预设 `preset` 没开功能 `feature`：交回 `key` 那一句，带上预设的名字；开着的、读不出预设的没有。
async fn preset_off(
    core: &Core,
    peer: &Peer,
    preset: String,
    feature: &str,
    key: &'static str,
) -> Option<Said> {
    let found = crate::presets::resolve(core, Some(&preset)).await.ok()?;
    if found.file.opens(feature) {
        return None;
    }
    let name = crate::personas::label(found.file.name.as_ref(), peer.language).unwrap_or(preset);
    Some(Said {
        key,
        fields: vec![("preset", name)],
    })
}

/// 记忆为什么用不了：人格记忆没装的、没有人格的、预设没开记忆的；会话表里查不到、说不准的是 `None`，照拒绝的那一句。
async fn memory(core: &Core, peer: &Peer, handle: &Handle) -> Option<Said> {
    if !crate::memory::installed(core) {
        return Some(Said::plain("commands/unavailable/not-installed"));
    }
    let listed = listed(core, handle).await?;
    if listed.persona.is_none() {
        return Some(Said::plain("commands/unavailable/no-persona"));
    }
    preset_off(
        core,
        peer,
        listed.preset?,
        MEMORY,
        "commands/unavailable/preset-off",
    )
    .await
}

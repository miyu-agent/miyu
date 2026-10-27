//! 用不了的命令为什么用不了（施工 O-6 再补，2026-10-09 项目主人在网页上验收：带了人格、预设没开记忆的会话打 `/remember`，
//! 不知道为什么）：`command.run` 拒绝的 `data.why`，照连接的语言。记忆用不了的说具体：会话的预设没开记忆，带上预设的名字，人
//! 才知道去哪改。长期目标用不了的（施工 X-8 下）同样说具体：预设没开；群里、私聊、子代理照拒绝的那一句。别的照拒绝的那一句。
//! 包没装的（施工 BS-1 下下起）没有这条命令，不走这里。
//!
//! 核心自己说的出错原因同样照连接的语言（施工 X-39 补）：提示词缺了哪几个参数，扩展的命令回答里没有 `said`。服务、扩展回的原话
//! 照原样，不在这里。

use miyu_policy::preset::MEMORY;
use miyu_session::Handle;

use super::{Said, words};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 长期目标这个包、功能的编号（同 `miyu_goal::PACKAGE`；端点不依赖软件包的 crate）。
pub(super) const GOAL_PACKAGE: &str = "goal";

/// 提示词缺了必填的参数：几个名字照这门语言的 `commands/joiner` 连起来。
pub(super) fn missing_arguments(missing: Vec<String>) -> Said {
    let names = missing
        .into_iter()
        .map(|name| Said::with("commands/failed/argument", vec![("name", name)]))
        .collect();
    Said {
        key: "commands/failed/missing-arguments",
        fields: Vec::new(),
        lists: vec![("names", names)],
    }
}

/// 扩展的命令 `name` 的回答里没有 `said`。
pub(super) fn no_reply(name: &str) -> Said {
    Said::with("commands/failed/no-reply", vec![("name", name.to_string())])
}

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

/// 长期目标为什么用不了（施工 X-8 下）：本机的主会话预设没开的；别的（群里、私聊、子代理，说不准的）是 `None`。没装的
/// （施工 BS-1 下下起）没有这条命令，走不到这里。
async fn goal(core: &Core, peer: &Peer, handle: &Handle) -> Option<Said> {
    let listed = listed(core, handle).await?;
    if handle.venue().as_str() != crate::list::LOCAL || listed.parent.is_some() {
        return None;
    }
    preset_off(
        core,
        peer,
        handle.owner(),
        listed.preset?,
        GOAL_PACKAGE,
        "commands/unavailable/goal-preset-off",
    )
    .await
}

/// 会话表里的这一条；查不到的没有。
pub(super) async fn listed(core: &Core, handle: &Handle) -> Option<crate::list::Listed> {
    let owner = handle.owner().clone();
    let (root, index) = (core.root.clone(), core.index_for(&owner));
    let id = handle.id().clone();
    tokio::task::spawn_blocking(move || crate::list::one(&root, &owner, &index, &id, false))
        .await
        .ok()?
}

/// 属主是 `owner` 的会话的预设 `preset` 没开功能 `feature`：交回 `key` 那一句，带上预设的名字；开着的、读不出预设的没有。
async fn preset_off(
    core: &Core,
    peer: &Peer,
    owner: &miyu_kernel::id::AccountId,
    preset: String,
    feature: &str,
    key: &'static str,
) -> Option<Said> {
    // 照会话的属主找（施工 Y-2 中上）。
    let found = crate::presets::resolve(core, owner, Some(&preset))
        .await
        .ok()?;
    if found.file.opens(feature) {
        return None;
    }
    let name = crate::personas::label(found.file.name.as_ref(), peer.language).unwrap_or(preset);
    Some(Said {
        key,
        fields: vec![("preset", name)],
        lists: Vec::new(),
    })
}

/// 记忆为什么用不了：预设没开记忆的（无人格的也有记忆，施工 P-12）；会话表里查不到、说不准的是 `None`，照拒绝的那一句。人格
/// 记忆没装的（施工 BS-1 下下起）没有这几条命令，走不到这里。
async fn memory(core: &Core, peer: &Peer, handle: &Handle) -> Option<Said> {
    let listed = listed(core, handle).await?;
    preset_off(
        core,
        peer,
        handle.owner(),
        listed.preset?,
        MEMORY,
        "commands/unavailable/preset-off",
    )
    .await
}

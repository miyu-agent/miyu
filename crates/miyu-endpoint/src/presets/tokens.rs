//! 预设列表里每个预设带的 token 数（施工 DM-4，2026-11-09 发版前检查第 4 条：新建会话时头在右下角写选的预设占多少）：新会话
//! 照它开、不算人格的那几块，即 system 里人设、场所、群聊以外的，加上工具的说明和参数格式。照不写会话、无人格的
//! `session.context` 那一份备、估（同一个本地估算器），只备不造。
//!
//! 每次列都现算，不存：release 下备、估一个预设 1～2 ms（2026-11-09 在出厂的基础系统上量），存了反倒要管配置、装的包、
//! 工具架子换代以后作废。

use std::sync::Arc;

use miyu_session::{Context, SystemPart};

use crate::Core;
use crate::hello::Peer;
use crate::session_tools::new_session;

/// 预设 `preset` 的 token 数：备不出来的没有。
pub(super) async fn of(core: &Arc<Core>, peer: Peer, preset: &str) -> Option<u64> {
    let mut setup = new_session(core, peer, Some(None), Some(preset.to_string()))
        .await
        .ok()?;
    let context = miyu_session::preview_context(setup.create(core))
        .await
        .ok()?;
    Some(fixed(&context))
}

/// 不随人格、场所变的：system 去掉人设、场所说明、群聊格式，加上工具。
fn fixed(context: &Context) -> u64 {
    let system: u64 = context
        .system
        .iter()
        .filter(|(part, _)| {
            !matches!(
                part,
                SystemPart::Persona | SystemPart::Venue | SystemPart::Group
            )
        })
        .map(|(_, tokens)| tokens)
        .sum();
    let tools: u64 = context
        .breakdown
        .tools
        .iter()
        .map(|(_, tokens)| tokens)
        .sum();
    system.saturating_add(tools)
}

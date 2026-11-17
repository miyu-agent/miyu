//! 扩展的工具交回的语音（施工 SD-1，项目主人 2026-11-17 本机验收第 16、26 条，say 交付方案「核心」一节）：`tool.call` 的回应里
//! `sent` 多收一种：写了 `"voice": true` 的一段音频。
//!
//! - 写法（[`shaped`]）：图至多 4 张照旧，语音至多一段，`media_type` 要是 `audio/` 开头的；多了、不对的整份当写法不对。
//! - 大小（[`fits`]）：一段至多 [`MOST_BYTES`]（10 MiB，mp3 十来分钟、16 位单声道 wav 三四分钟），大了的不交，回她一句
//!   （`core/tool-results/voice-too-big.txt`）。
//! - 兜底放（[`fallback`]）：本机会话里没人开口的一轮（到点的定时任务、循环、人生叫醒的，没有触发的那句的）出了语音，这一刻
//!   又没有一个头开着这个会话（`session.viewing`），核心经总线调 `tts.play {"audio": <blob>}`，从这台电脑的音箱放；有头开着的由
//!   头放，场所会话由桥发。没人提供 `tts` 的记一行，语音照旧在日志里。

use std::sync::Weak;

use serde_json::json;

use miyu_kernel::event::Sent;
use miyu_kernel::id::ContentHash;
use miyu_kernel::origin::By;
use miyu_store::blob::Blobs;
use miyu_tool::{Call, Done};

use crate::Core;

/// 一次至多几段语音。
const MOST_VOICES: usize = 1;

/// 一段语音至多多大，字节。
pub(super) const MOST_BYTES: u64 = 10 * 1024 * 1024;

/// 交回的 `sent` 写法对不对：图至多 `images` 张，语音至多一段、`audio/` 开头。
pub(super) fn shaped(sent: &[Sent], images: usize) -> bool {
    let voices: Vec<_> = sent
        .iter()
        .filter_map(|sent| match sent {
            Sent::Voice(voice) => Some(voice),
            Sent::Image(_) => None,
        })
        .collect();
    sent.len() - voices.len() <= images
        && voices.len() <= MOST_VOICES
        && voices
            .iter()
            .all(|voice| voice.media_type.as_str().starts_with("audio/"))
}

/// 语音的 blob。
pub(super) fn blobs(sent: &[Sent]) -> Vec<ContentHash> {
    sent.iter()
        .filter(|sent| matches!(sent, Sent::Voice(_)))
        .map(|sent| sent.blob().clone())
        .collect()
}

/// 这几段都不大过 [`MOST_BYTES`]（读不到大小的当不大：在不在由别处查）。读盘，在阻塞线程里调。
pub(super) fn fits(blobs: &Blobs, voices: &[ContentHash]) -> bool {
    voices.iter().all(|hash| {
        std::fs::metadata(blobs.path(hash)).map_or(true, |meta| meta.len() <= MOST_BYTES)
    })
}

/// 大过上限那一句的字段：上限写成几 MB。
pub(super) fn megabytes() -> String {
    (MOST_BYTES / (1024 * 1024)).to_string()
}

/// 这一轮没人开口：到点叫醒的，或者没有触发的那句。
fn unattended(asked: Option<&By>) -> bool {
    matches!(asked, None | Some(By::Schedule(_)))
}

/// 见模块的说明：该兜底放的在后台交给 `tts.play`，不等它放完。
pub(super) fn fallback(core: &Weak<Core>, call: &Call, done: &Done) {
    if done.error {
        return;
    }
    let Some(voice) = done.sent.iter().find(|sent| matches!(sent, Sent::Voice(_))) else {
        return;
    };
    let Some(ids) = call.ids.as_ref() else {
        return;
    };
    let local = call
        .caller
        .as_ref()
        .is_some_and(|caller| caller.venue.as_str() == crate::list::LOCAL);
    if !local || !unattended(ids.asked.as_ref()) {
        return;
    }
    let Some(core) = core.upgrade() else {
        return;
    };
    if core.viewing.ids().contains(&ids.session) {
        return;
    }
    let blob = voice.blob().clone();
    tokio::spawn(async move {
        let params = json!({"audio": blob});
        match crate::backstage::bus::call_by_core(&core, ("tts", "play"), params).await {
            Ok(_) => tracing::info!(target: "miyu::endpoint", "voice played here"),
            Err(refusal) => {
                tracing::warn!(target: "miyu::endpoint", reason = refusal.reason, "voice not played");
            }
        }
    });
}

#[cfg(test)]
mod tests;

//! 中转把上游的拒绝说明当正文流回来（施工 AG-1）：Google 的内容政策拦下请求时，有的中转（Antigravity 那一路）把「The prompt
//! could not be submitted. …」当成一段普通的 `delta.content` 流出来，接着发一个出错事件、流断了。照原样走，这段字会落成她说的
//! 话（被打断的半截）。
//!
//! 认法尽量窄：回复的第一段正文里，拒绝的开头那一句和 Google 政策的链接**同时**在，才扣住（[`Decoder`](super::Decoder)）；扣着的
//! 时候紧跟着来的是出错事件、流断了，整段丢掉，整次照内容策略出错；来的是别的（下一段正文、`finish_reason`、`[DONE]`），照原样
//! 交出去，只晚一个分块。
//!
//! - 误杀面：一个真模型的回复第一段正好同时有这一句和这个链接、紧接着流又出错，这一段才丢；那一轮本来就是出错。
//! - 漏认面：Google 换了说法、别的语言、拒绝说明分成几块到、别的中转换了包法，都认不出，照旧落成被打断的半截。

use miyu_kernel::event::{CallError, ErrorClass};

/// 拒绝说明开头的那一句。
const OPENING: &str = "The prompt could not be submitted.";

/// Google 生成式 AI 禁用政策的链接（不带协议、`www`，照子串比）。
const POLICY_LINK: &str = "policies.google.com/terms/generative-ai/use-policy";

/// `text` 是中转转来的 Google 拒绝说明：开头那一句、政策链接都在。
pub(super) fn is_refusal(text: &str) -> bool {
    text.trim_start().starts_with(OPENING) && text.contains(POLICY_LINK)
}

/// 照内容策略出错：说明写拒绝的原话（最长 500 个字）。
pub(super) fn refused(text: &str) -> CallError {
    CallError {
        class: ErrorClass::ContentPolicy,
        message: text.trim().chars().take(500).collect(),
        status: None,
        retry_after_ms: None,
    }
}

#[cfg(test)]
mod tests;

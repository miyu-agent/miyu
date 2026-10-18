//! `memory.*` 的几样参数怎么查（施工 R-3 补；R-15 上加关键词，从 `memory.rs` 拆出来）：正文、关键词、条数。

use miyu_recall::KEYWORD_CHARS;
use miyu_tool::TEXT_CHARS;

use crate::refusal::Refusal;

/// `limit` 最多写几。
pub(super) const LIMIT: usize = 500;

/// 正文：去掉前后空白，空的参数不对，超过 [`TEXT_CHARS`] 个字的太长。
pub(super) fn checked(text: &str) -> Result<String, Refusal> {
    let text = text.trim();
    let chars = text.chars().count();
    match chars {
        0 => Err(Refusal::BAD_PARAMS),
        _ if chars > TEXT_CHARS => Err(Refusal::memory_too_long(chars, TEXT_CHARS, "text")),
        _ => Ok(text.to_string()),
    }
}

/// 关键词（施工 R-15 上）：去掉前后空白，空的是去掉（`memory.update`）、没有（`memory.remember`），超过 [`KEYWORD_CHARS`] 个字的
/// 太长。人给的照原样，不截。
pub(super) fn keywords(keywords: &str) -> Result<String, Refusal> {
    let keywords = keywords.trim();
    let chars = keywords.chars().count();
    if chars > KEYWORD_CHARS {
        return Err(Refusal::memory_too_long(chars, KEYWORD_CHARS, "keywords"));
    }
    Ok(keywords.to_string())
}

/// `limit`：不写的照 `default`，1 到 [`LIMIT`]，别的参数不对。
pub(super) fn limit(limit: Option<usize>, default: usize) -> Result<usize, Refusal> {
    match limit.unwrap_or(default) {
        limit @ 1..=LIMIT => Ok(limit),
        _ => Err(Refusal::BAD_PARAMS),
    }
}

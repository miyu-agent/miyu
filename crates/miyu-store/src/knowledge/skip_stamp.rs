//! 跳过的文档也记指纹（施工 W-15 再补）：读不了、太大、PDF 里没有字的文档，原来不记指纹，每次列库都当它变了，重读一遍（PDF
//! 还要重新抽字）、推一遍建索引的进度，网页收到推送重读列表又转一圈。记下以后文件没变的照记下的原因报跳过，不重读、不算。
//!
//! 写法：正常的指纹是 `大小:改的时刻`；跳过的是同一个指纹后面接原因的写法和为什么，用单元分隔符（`\u{1f}`）隔开，文件名、原因
//! 里都不会有它。没装抽字的小程序（`no_extractor`）那一种，装上了就不算数、重读。

use std::collections::{BTreeMap, BTreeSet};

use super::Reason;

/// 隔开指纹、原因、为什么。
const SEP: char = '\u{1f}';

/// 文件指纹是 `stamp`、因为 `reason`（`why`）跳过的那一份指纹。
pub(super) fn skipped(stamp: &str, reason: Reason, why: &str) -> String {
    format!("{stamp}{SEP}{}{SEP}{why}", reason.code())
}

/// 记着的 `recorded` 是不是文件指纹还是 `stamp` 时跳过的：是的交回原因和为什么。`extractor` 是这一刻有没有抽字的小程序：
/// 有了的，「没有抽字的小程序」那一份不算数。不是跳过的、文件变了的、认不得的没有。
pub(super) fn read(recorded: &str, stamp: &str, extractor: bool) -> Option<(Reason, String)> {
    let mut parts = recorded.splitn(3, SEP);
    let (Some(file), Some(code), Some(why)) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    let reason = reason_of(code)?;
    (file == stamp && !(extractor && reason == Reason::NoExtractor))
        .then(|| (reason, why.to_string()))
}

/// 照协议里的写法认回原因；不认识的没有。
fn reason_of(code: &str) -> Option<Reason> {
    [
        Reason::TooLarge,
        Reason::NotUtf8,
        Reason::NoText,
        Reason::NoExtractor,
        Reason::ExtractFailed,
        Reason::Unreadable,
    ]
    .into_iter()
    .find(|reason| reason.code() == code)
}

/// 进了索引的文档有几篇：指纹 `stamps` 里正常的，加上跳过了、还留着旧块的（块的键 `keys` 是 `<来源>#<第几块>`）；从没进过的
/// 跳过的不算（同记跳过以前的口径）。
pub(super) fn indexed(stamps: &BTreeMap<String, String>, keys: &[String]) -> usize {
    let kept: BTreeSet<&str> = keys
        .iter()
        .filter_map(|key| key.rsplit_once('#').map(|(source, _)| source))
        .collect();
    stamps
        .iter()
        .filter(|(source, stamp)| !stamp.contains(SEP) || kept.contains(source.as_str()))
        .count()
}

#[cfg(test)]
mod tests;

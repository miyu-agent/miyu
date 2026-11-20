//! 设置项命名的规矩（`internal/blueprint/config.md`「给人看的字」的「命名的规矩」，施工 UX-1，项目主人 2026-11-17）：名字是软件术语、名词短语，
//! 不写成一句话、不带「AI」「你」；开关一律正向，`true` 是开。
//!
//! 机器只拦写错了一眼就看得出的几样，别的靠人审：
//!
//! - 开关的键最后一段不以 `off`、`disable`、`disabled` 结尾，也不以 `no_`、`not_`、`disable_`、`hide_`、`skip_` 开头（要反着
//!   说的，键改成 `…enabled`，`true` 是开）；
//! - 项、组的名字不带句子里才有的字：人称、疑问、把字句、句末标点、「AI」；
//! - 开关的名字不带反着说的字（「不」「禁用」「隐藏」……）。
//!
//! 只查核心自己的字表和它登记的项（`miyu-core` 的 `tests/settings_names.rs`）；别人做的包照它自己的写法，不拦。

use crate::item::{Item, Kind};
use crate::words::ConfigWords;

/// 开关的键最后一段不许以这几个结尾。
const NEGATIVE_ENDS: [&str; 3] = ["off", "disable", "disabled"];

/// 开关的键最后一段不许以这几个开头。
const NEGATIVE_STARTS: [&str; 5] = ["no_", "not_", "disable_", "hide_", "skip_"];

/// 一种语言里名字不许带的：句子里才有的字。英文照整词比（不分大小写），别的照字比。
fn sentence_words(language: &str) -> &'static [&'static str] {
    match language {
        "zh" => &[
            "你", "您", "我", "吗", "把", "让", "怎么", "什么", "多久", "谁", "AI", "？", "。",
            "！", "，",
        ],
        "ja" => &["あなた", "自分", "AI", "？", "。", "！"],
        _ => &["you", "your", "me", "my", "ai", "?", "!"],
    }
}

/// 一种语言里开关的名字不许带的：反着说的字。
fn negative_words(language: &str) -> &'static [&'static str] {
    match language {
        "zh" => &["不", "禁用", "禁止", "关掉", "关闭", "隐藏", "停用"],
        "ja" => &["しない", "無効", "オフ", "隠す"],
        _ => &["disable", "don't", "hide", "off", "no", "not", "skip"],
    }
}

/// 查 `items` 和语言 `language` 的字 `words`，交回每一处不合规矩的，一处一句。字表里没有的项不在这里报（[`crate::words::check`]
/// 报）。
pub fn check(items: &[Item], words: &ConfigWords, language: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for item in items.iter().filter(|item| item.kind == Kind::Bool) {
        let last = item.key.rsplit('.').next().unwrap_or(item.key);
        if NEGATIVE_ENDS.iter().any(|end| last.ends_with(end))
            || NEGATIVE_STARTS.iter().any(|start| last.starts_with(start))
        {
            problems.push(format!(
                "{}：开关的键要正着说（改成 …enabled，true 是开）",
                item.key
            ));
        }
        if let Some(said) = words.items.get(item.key)
            && let Some(found) = found(&said.name, negative_words(language), language)
        {
            problems.push(format!(
                "{} 的名字「{}」反着说：带了「{found}」",
                item.key, said.name
            ));
        }
    }
    let names = words
        .items
        .iter()
        .map(|(key, said)| (key.as_str(), said.name.as_str()))
        .chain(
            words
                .groups
                .iter()
                .map(|(id, name)| (id.as_str(), name.as_str())),
        );
    for (at, name) in names {
        if let Some(found) = found(name, sentence_words(language), language) {
            problems.push(format!("{at} 的名字「{name}」像一句话：带了「{found}」"));
        }
    }
    problems
}

/// `name` 里带了 `banned` 的哪一个：英文照整词比，别的照字比。
fn found(name: &str, banned: &[&'static str], language: &str) -> Option<&'static str> {
    let english = !matches!(language, "zh" | "ja");
    let lower = name.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|word| !word.is_empty())
        .collect();
    banned.iter().copied().find(|banned| {
        let lowered = banned.to_lowercase();
        match english && lowered.chars().all(|c| c.is_alphanumeric() || c == '\'') {
            true => words.contains(&lowered.as_str()),
            false => name.contains(banned),
        }
    })
}

#[cfg(test)]
mod tests;

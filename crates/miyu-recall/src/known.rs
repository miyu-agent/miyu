//! 搜人卡（施工 E-3，设计 `42-认识的人.md` 第三节第 4 条，`docs/blueprint/memory.md`「工具」）：她的 `memory_search` 照一句话
//! 找得到认识的人的卡，名字、外号、平台号都算关键词。
//!
//! 卡不进记忆库（库里是一条条记忆），人数也少（一间几十张），这里照底账里的卡当场对：
//!
//! 1. **对上是谁**：卡的平台编号（`qq:10086`）整个出现在这句话里，或者编号冒号后面那截（`10086`）是这句话的一个词；卡的叫法
//!    （群名片、外号）整个出现在这句话里（不分大小写，一个字的叫法不算：一个字太容易碰上）；短叫法（去掉空白不超过
//!    [`SHORT_NAME`] 个字）里落着这句话的一个两字词（[`query_terms`]，「你认识小林吗」的「小林」对「小林子」）；这句话的一个
//!    英文、数字词是叫法里的一个词（`rust` 对 `GNU/Rust++`）。长叫法多半是一句话的群名片（「我不是MoYingJi」「今天也要开心」），
//!    拿两字词去落会让「我不是这个意思」「你今天开心吗」碰上不相干的人，只认整个出现、英文数字词。对上的照对上的那截有多长排
//!    （编号最先），长的在前。
//! 2. **对上正文**：一张都没对上是谁的，才照正文找：这句话的两个字以上的词（一半以上的卡都有的不算：「认识」「群里」谁的卡都
//!    写）在卡的正文里至少对上两个，而且其中有挨着的两个连成一截也在正文里（「搬去大阪」），或者有一个英文、数字词
//!    （「Rust」）；多的在前。卡少的时候「一半以上」挡不住常用词，零散对上的「喜欢」「什么」不算，连成一截、带专名的才像是在说
//!    这个人。问的是某个人的，只给他（们）的卡，别人卡里碰巧有同一个词的不混进来。
//! 3. 一样的照写下的先后，新的在前；最多交 `limit` 张。作废的卡调的一方先挑掉。

use std::cmp::Reverse;

use crate::memory::PersonCard;
use crate::terms::{is_cjk, query_terms};

/// 平台编号整个出现在这句话里时算的长度：比任何叫法都长，排最先。
const WHOLE_ID: usize = usize::MAX;

/// 短叫法最多几个字（去掉空白）：两字词落在它里面才算对上。人名、外号多半两三个字，四个字的也有（「欧阳娜娜」）；两字词至少占
/// 叫法的一半，和「不超过四个字」是一回事。
const SHORT_NAME: usize = 4;

/// 照正文找时一张卡至少对上几个词。
const TOLD: usize = 2;

/// 照 `query` 找卡（`cards` 是现在算数的那几张），最多 `limit` 张，最相关的在前。
pub fn find_cards<'a>(query: &str, cards: &'a [PersonCard], limit: usize) -> Vec<&'a PersonCard> {
    let said = query.to_lowercase();
    let terms = query_terms(query);
    let mut named: Vec<(usize, &PersonCard)> = cards
        .iter()
        .filter_map(|card| named(card, &said, &terms).map(|length| (length, card)))
        .collect();
    if !named.is_empty() {
        named.sort_by_key(|(length, card)| (Reverse(*length), Reverse(card.at)));
        return named
            .into_iter()
            .take(limit)
            .map(|(_, card)| card)
            .collect();
    }
    let texts: Vec<String> = cards.iter().map(|card| card.text.to_lowercase()).collect();
    let rare: Vec<&String> = terms
        .iter()
        .filter(|term| term.chars().count() >= 2)
        .filter(|term| {
            let having = texts
                .iter()
                .filter(|text| text.contains(term.as_str()))
                .count();
            having * 2 <= cards.len() + 1
        })
        .collect();
    let mut told: Vec<(usize, &PersonCard)> = cards
        .iter()
        .zip(&texts)
        .filter_map(|(card, text)| told(&rare, text).map(|hits| (hits, card)))
        .collect();
    told.sort_by_key(|(hits, card)| (Reverse(*hits), Reverse(card.at)));
    told.into_iter().take(limit).map(|(_, card)| card).collect()
}

/// 照正文对上了几个词（`rare` 是这句话里少见的词，照这句话里的先后；`text` 是转成小写的正文）：不到 [`TOLD`] 个的、对上的
/// 既没有挨着连成一截也没有英文数字词的，不算。
fn told(rare: &[&String], text: &str) -> Option<usize> {
    let hits: Vec<&String> = rare
        .iter()
        .copied()
        .filter(|term| text.contains(term.as_str()))
        .collect();
    // 挨着的两个两字词（「搬去」「去大」）首尾那个字一样，连成三个字（「搬去大」）也在正文里。
    let joined = hits.windows(2).any(|pair| {
        let (first, second) = (pair[0], pair[1]);
        let tail = second.chars().nth(1);
        second.chars().count() == 2
            && first.chars().nth(1) == second.chars().next()
            && tail.is_some_and(|tail| text.contains(&format!("{first}{tail}")))
    });
    let word = hits
        .iter()
        .any(|term| term.chars().all(|c| c.is_ascii_alphanumeric()));
    (hits.len() >= TOLD && (joined || word)).then_some(hits.len())
}

/// 这句话（`said` 是转成小写的原句，`terms` 是切出的词）对上这张卡的是谁的那截有多长；没对上的没有。
fn named(card: &PersonCard, said: &str, terms: &[String]) -> Option<usize> {
    let who = card.who.to_lowercase();
    if said.contains(&who) {
        return Some(WHOLE_ID);
    }
    let mut best = None;
    let mut take = |length: usize| best = Some(best.map_or(length, |best: usize| best.max(length)));
    if let Some((_, number)) = who.rsplit_once(':')
        && !number.is_empty()
        && terms.iter().any(|term| term == number)
    {
        take(number.chars().count());
    }
    for name in &card.names {
        let name = name.trim().to_lowercase();
        let length = name.chars().count();
        let short = name.chars().filter(|c| !c.is_whitespace()).count() <= SHORT_NAME;
        if length >= 2 && said.contains(&name) {
            take(length);
            continue;
        }
        let words: Vec<&str> = name
            // 汉字假名也断开：「我不是MoYingJi」的英文词是「moyingji」。
            .split(|c: char| !c.is_alphanumeric() || is_cjk(c))
            .filter(|word| !word.is_empty())
            .collect();
        for term in terms {
            let cjk = term.chars().count() == 2 && term.chars().all(|c| !c.is_ascii());
            if (short && cjk && name.contains(term.as_str())) || words.contains(&term.as_str()) {
                take(term.chars().count());
            }
        }
    }
    best
}

#[cfg(test)]
mod tests;

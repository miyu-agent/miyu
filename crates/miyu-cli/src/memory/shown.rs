//! 一条记忆印成一行（`docs/blueprint/cli/memory.md`「样子」）：`<编号>  <日期>  <正文>`，编号照最长的那个对齐；日期照这台
//! 机器此刻的时区换成那一天；作废的后面接一截。只放人要的：出处、谁记的、类不印（2026-10-08 项目主人定的设计原则）。认识的人
//! 那一类正文前面写是谁（施工 E-1 上）：`<编号>  <日期>  [<是谁>] <正文>`；发生过的事涉及谁的同样写在前面（施工 E-1 修四）。人卡（施工 E-1 下）一张一行：`<是谁>  <日期>  <正文>`，
//! 有叫法的是谁后面括号里接上。

use serde_json::Value;

use miyu_kernel::time::{Timestamp, UtcOffset};

use crate::language::Language;

/// 核心回的那一串（`memory.list`、`memory.search` 的 `memories`），一条一行，不带换行。读不懂的时刻照原样写。
pub(crate) fn listed(memories: &[Value], offset: UtcOffset, language: &Language) -> Vec<String> {
    let id = |memory: &Value| memory["id"].as_str().unwrap_or_default().to_string();
    let width = memories
        .iter()
        .map(|memory| id(memory).len())
        .max()
        .unwrap_or(0);
    memories
        .iter()
        .map(|memory| {
            let at = memory["at"].as_str().unwrap_or_default();
            let day =
                Timestamp::parse(at).map_or_else(|_| at.to_string(), |at| at.local_date(offset));
            let text = memory["text"].as_str().unwrap_or_default();
            // 认识的人那一类先写是谁（施工 E-1 上），发生过的事先写涉及谁（施工 E-1 修四）。
            let people: Vec<&str> = memory["people"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let mut row = match (memory["who"].as_str(), people.is_empty()) {
                (Some(who), _) => format!("{:width$}  {day}  [{who}] {text}", id(memory)),
                (None, false) => {
                    format!(
                        "{:width$}  {day}  [{}] {text}",
                        id(memory),
                        people.join(", ")
                    )
                }
                (None, true) => format!("{:width$}  {day}  {text}", id(memory)),
            };
            if let Some(why) = memory["retired"].as_str() {
                row.push_str(&language.forgotten_mark(why));
            }
            row
        })
        .collect()
}

/// 核心回的人卡（`memory.cards` 的 `cards`），一张一行：`<是谁>（<叫法>）  <日期>  <正文>`，是谁照最长的对齐；作废的后面接一截。
pub(crate) fn cards(cards: &[Value], offset: UtcOffset, language: &Language) -> Vec<String> {
    let head = |card: &Value| {
        let who = card["who"].as_str().unwrap_or_default();
        let names: Vec<&str> = card["names"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        if names.is_empty() {
            who.to_string()
        } else {
            format!("{who} ({})", names.join(", "))
        }
    };
    let width = cards
        .iter()
        .map(|card| head(card).chars().count())
        .max()
        .unwrap_or(0);
    cards
        .iter()
        .map(|card| {
            let at = card["at"].as_str().unwrap_or_default();
            let day =
                Timestamp::parse(at).map_or_else(|_| at.to_string(), |at| at.local_date(offset));
            let head = head(card);
            let pad = " ".repeat(width - head.chars().count());
            let text = card["text"].as_str().unwrap_or_default();
            let mut row = format!("{head}{pad}  {day}  {text}");
            if let Some(why) = card["retired"].as_str() {
                row.push_str(&language.forgotten_mark(why));
            }
            row
        })
        .collect()
}

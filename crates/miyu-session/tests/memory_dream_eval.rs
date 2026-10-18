//! 量尺：整理（施工 R-16，`docs/blueprint/memory/eval.md`「整理」）：虚构的林夏（`fixtures/dream-eval/set.toml`），每条标好该怎么
//! 处理，照真的整理（拼法、相关的旧记忆、核对都是会话里那一套）跑开发端点的模型，对着标注打分。两种场景：第一次（还没有摘要，
//! 40 条一起）；平时（先记旧的、整理一次有了摘要，再记新的、整理第二次，只看第二次）。
//!
//! 跑：`MIYU_DEV_BASE_URL=… MIYU_DEV_API_KEY=… MIYU_DEV_MODEL=… cargo test -p miyu-session --test all memory_dream_eval --
//! --ignored --nocapture`；`MIYU_DREAM_EVAL_ROUNDS` 是每种场景几轮（不写是 3）。没设的什么都不量。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;

use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::{MemoryEvent, MemoryId, Saved};
use miyu_session::{Keeper, Listener, Turn};

use crate::support::extracting::organizer_dev;
use crate::support::meaning::persona;
use crate::support::*;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    owner: String,
    memory: Vec<Memory>,
    question: Vec<Question>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Memory {
    key: String,
    date: String,
    class: String,
    text: String,
    label: String,
    #[serde(default)]
    by: Option<String>,
    #[serde(default)]
    of: Option<String>,
    #[serde(default)]
    want: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Question {
    text: String,
    expect: Vec<String>,
}

fn set() -> Set {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dream-eval/set.toml");
    toml_edit::de::from_str(&std::fs::read_to_string(path).expect("读得到")).expect("合写法")
}

#[test]
fn the_set_is_well_formed() {
    let set = set();
    assert_eq!(set.memory.len(), 40);
    let keys: Vec<&str> = set
        .memory
        .iter()
        .map(|memory| memory.key.as_str())
        .collect();
    for memory in &set.memory {
        let wants = match memory.label.as_str() {
            "keep" | "worthless" => true,
            "old" => memory.by.as_deref().is_some_and(|by| keys.contains(&by)),
            "dup" => memory.of.as_deref().is_some_and(|of| keys.contains(&of)),
            "date" => memory.want.is_some(),
            _ => false,
        };
        assert!(wants, "{memory:?}");
    }
    assert_eq!(set.question.len(), 8);
}

/// 这一条记下的那一刻：那一天的正午（东九区）。
fn day(date: &str) -> Timestamp {
    Timestamp::parse(&format!("{date}T03:00:00.000Z")).expect("合写法")
}

/// 照先后记下 `memories`，交回 键 → 编号。
fn remember(home: &Home, memories: &[&Memory], ids: &mut BTreeMap<String, MemoryId>) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    for memory in memories {
        let saved = Saved {
            class: memory.class.clone(),
            text: memory.text.clone(),
            sources: Vec::new(),
            audience: vec![alice()],
            replaces: None,
            about: None,
            venue: None,
            keywords: None,
        };
        let appended = log
            .append(day(&memory.date), alice(), None, &MemoryEvent::Saved(saved))
            .expect("记得下");
        ids.insert(memory.key.clone(), appended.id);
    }
}

async fn dream(home: &Home) {
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    let dreamed = keeper
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await;
    println!("  整理：{dreamed:?}");
}

/// 一轮的分：矛盾处理、日期、没价值、重复、误删、摘要答得上。
#[derive(Debug, Default)]
struct Score {
    old: usize,
    date: usize,
    worthless: usize,
    dup: usize,
    wrong: Vec<String>,
    answered: usize,
}

fn score(home: &Home, set: &Set, ids: &BTreeMap<String, MemoryId>) -> Score {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.book(|book| {
        let gone = |key: &str| {
            book.get(ids[key])
                .is_some_and(|entry| entry.retired.is_some())
        };
        let changed = |key: &str| book.get(ids[key]).and_then(|entry| entry.replaced_by);
        let now_text = |key: &str| {
            changed(key)
                .and_then(|id| book.get(id))
                .map(|entry| entry.text.clone())
                .unwrap_or_default()
        };
        let mut score = Score::default();
        for memory in &set.memory {
            let key = memory.key.as_str();
            match memory.label.as_str() {
                "old" => score.old += usize::from(gone(key) || changed(key).is_some()),
                "date" => {
                    let want = memory.want.as_deref().unwrap_or_default();
                    let (year, rest) = want.split_at(4);
                    let month: u32 = rest[1..3].parse().unwrap_or(0);
                    let date: u32 = rest[4..6].parse().unwrap_or(0);
                    let text = now_text(key);
                    let state = if gone(key) {
                        "作废了".to_string()
                    } else if text.is_empty() {
                        "没改".to_string()
                    } else {
                        text.clone()
                    };
                    println!("  要改成 {want}：{} → {state}", memory.text);
                    score.date += usize::from(
                        text.contains(want) || text.contains(&format!("{year}年{month}月{date}日")),
                    );
                }
                "worthless" => score.worthless += usize::from(gone(key)),
                "dup" => {
                    let of = memory.of.as_deref().unwrap_or_default();
                    score.dup += usize::from(gone(key) != gone(of));
                }
                _ => {
                    let partner = set
                        .memory
                        .iter()
                        .find(|other| other.of.as_deref() == Some(key))
                        .map(|other| other.key.as_str());
                    if gone(key) && partner.is_none_or(gone) {
                        score.wrong.push(memory.text.chars().take(16).collect());
                    }
                }
            }
        }
        let summary = book
            .summary()
            .map(|(text, _)| text.to_string())
            .unwrap_or_default();
        for question in &set.question {
            if question
                .expect
                .iter()
                .all(|word| summary.contains(word.as_str()))
            {
                score.answered += 1;
            } else {
                println!("  摘要答不上：{}", question.text);
            }
        }
        println!("  摘要：{summary}");
        score
    })
}

fn count(set: &Set, label: &str) -> usize {
    set.memory
        .iter()
        .filter(|memory| memory.label == label)
        .count()
}

#[tokio::test]
#[ignore = "量尺：要设 MIYU_DEV_BASE_URL、MIYU_DEV_API_KEY、MIYU_DEV_MODEL"]
async fn measure_the_dream_eval_set() {
    let set = set();
    let rounds: usize = std::env::var("MIYU_DREAM_EVAL_ROUNDS")
        .ok()
        .and_then(|rounds| rounds.parse().ok())
        .unwrap_or(3);
    let personal = format!("[profile]\nname = \"{}\"\n", set.owner);
    let newer: Vec<&str> = set
        .memory
        .iter()
        .filter(|memory| matches!(memory.label.as_str(), "dup" | "date" | "worthless"))
        .map(|memory| memory.key.as_str())
        .chain(set.memory.iter().filter_map(|memory| memory.by.as_deref()))
        .collect();
    for scenario in ["第一次", "平时"] {
        let mut total = Score::default();
        for round in 1..=rounds {
            let mut home = Home::new();
            if !organizer_dev(&mut home, &personal) {
                println!("没设 MIYU_DEV_BASE_URL、MIYU_DEV_API_KEY、MIYU_DEV_MODEL：不量");
                return;
            }
            println!("{scenario} 第 {round} 轮");
            let mut ids = BTreeMap::new();
            if scenario == "第一次" {
                remember(&home, &set.memory.iter().collect::<Vec<_>>(), &mut ids);
            } else {
                let old: Vec<&Memory> = set
                    .memory
                    .iter()
                    .filter(|memory| !newer.contains(&memory.key.as_str()))
                    .collect();
                remember(&home, &old, &mut ids);
                dream(&home).await;
                let new: Vec<&Memory> = set
                    .memory
                    .iter()
                    .filter(|memory| newer.contains(&memory.key.as_str()))
                    .collect();
                remember(&home, &new, &mut ids);
            }
            dream(&home).await;
            let one = score(&home, &set, &ids);
            println!("  {one:?}");
            total.old += one.old;
            total.date += one.date;
            total.worthless += one.worthless;
            total.dup += one.dup;
            total.wrong.extend(one.wrong);
            total.answered += one.answered;
        }
        println!(
            "{scenario}（{rounds} 轮）：矛盾处理 {}/{} 日期 {}/{} 没价值 {}/{} 重复 {}/{} 误删 {} {:?} 摘要答得上 {}/{}",
            total.old,
            count(&set, "old") * rounds,
            total.date,
            count(&set, "date") * rounds,
            total.worthless,
            count(&set, "worthless") * rounds,
            total.dup,
            count(&set, "dup") * rounds,
            total.wrong.len(),
            total.wrong,
            total.answered,
            set.question.len() * rounds
        );
    }
}

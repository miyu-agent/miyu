//! `memory_search`（施工 R-3 中）：搜记下的、认识的人的卡（施工 E-3）和以前的对话。记下的一条一行在前，卡一张一行在中间，
//! 以前的对话一条一行在后；什么都没有说一句。挑哪些（现在算数的、出处活着的、听众合的）是端口的事，这里只照端口交回的写。
//!
//! 卡排在记下的后面：用到几次照开头那几行的编号数（`miyu_recall::uses`，头一个词不是编号的那一行就停），卡排前面就数不到了。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::Access;
use miyu_recall::shown_class;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{
    Call, Done, FoundCard, Line as MemoryLine, MEMORY_SEARCH, Progress, Running, Searched, Spec,
    Tool,
};

use crate::texts::{Texts, line, said};
use crate::{SOFTWARE, TURN_CHARS};

/// `memory_search`。
pub(crate) struct Search {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    query: String,
    #[serde(default)]
    forgotten: bool,
}

impl Search {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Search, LoadError> {
        Ok(Search {
            spec: load::spec(resources, SOFTWARE, MEMORY_SEARCH, Access::Read)?,
            texts,
        })
    }
}

impl Tool for Search {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let all = texts.all();
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.bad_args(&error),
            };
            let Some(port) = call.memory.as_ref() else {
                return texts.off();
            };
            let found = match port.search(args.query, args.forgotten).await {
                Ok(found) => found,
                Err(error) => return texts.refused(&miyu_tool::Refused::Failed(error)),
            };
            let shown = detail(&found, call.offset);
            if found.memories.is_empty() && found.turns.is_empty() && found.cards.is_empty() {
                return Done::ok(line(&all.nothing, &[]))
                    .said(said("memory_search/nothing", &[]))
                    .data(shown.clone())
                    .detail(shown);
            }
            let mut lines = Vec::new();
            for memory in &found.memories {
                let (id, date) = (memory.id.to_string(), memory.at.local_date(call.offset));
                let class = shown_class(&memory.class, memory.who.as_deref());
                lines.push(
                    all.lines
                        .say(&MemoryLine {
                            id: &id,
                            class: &class,
                            date: &date,
                            text: &memory.text,
                            venue: memory.venue.as_ref(),
                            place: memory.place.as_deref(),
                            retired: memory.retired,
                        })
                        .trim_end()
                        .to_string(),
                );
            }
            for card in &found.cards {
                lines.push(line(
                    &all.card,
                    &[("who", &who(card)), ("text", &card.text.replace('\n', " "))],
                ));
            }
            // 以前的对话：她读得了别的会话（有 `sessions` 的端口，本机的主会话才有）、又打得开那一个的才给编号，`history` 照它
            // 读得到；别的（群会话、分叉出来的支线、别人的，场所会话里搜的一律）不给编号，写在哪聊的（施工 E-6）。
            let reads = call.sessions.is_some();
            for turn in &found.turns {
                let date = turn.at.local_date(call.offset);
                let text = flat(&turn.text);
                lines.push(match reads && turn.opens {
                    true => line(
                        &all.turn,
                        &[
                            ("date", &date),
                            ("session", turn.session.short()),
                            ("text", &text),
                        ],
                    ),
                    false => line(
                        &all.turn_elsewhere,
                        &[
                            ("date", &date),
                            ("where", &all.lines.place(turn.venue.as_ref(), None)),
                            ("text", &text),
                        ],
                    ),
                });
            }
            let (memories, turns, cards) = (
                found.memories.len().to_string(),
                found.turns.len().to_string(),
                found.cards.len().to_string(),
            );
            // 有卡的另一句（施工 E-3）：没卡的照旧，以前的样子不变。
            let human = match found.cards.is_empty() {
                true => said(
                    "memory_search/found",
                    &[("memories", &memories), ("turns", &turns)],
                ),
                false => said(
                    "memory_search/found-cards",
                    &[
                        ("cards", &cards),
                        ("memories", &memories),
                        ("turns", &turns),
                    ],
                ),
            };
            Done::ok(lines.join("\n"))
                .said(human)
                .data(shown.clone())
                .detail(shown)
        })
    }
}

/// 给头看的那一份（施工 W-11，`docs/blueprint/memory.md`「工具」）：记忆、对话片段照给她看的先后，日期照会话的时区 `offset`；
/// 作废的带 `forgotten`，对话片段是原文（不截、不换行），会话写整个编号；有卡的多 `cards`（是谁、叫法、日期、原文，施工 E-3）。
fn detail(found: &Searched, offset: UtcOffset) -> serde_json::Value {
    let memories: Vec<serde_json::Value> = found
        .memories
        .iter()
        .map(|memory| {
            let mut item = serde_json::json!({
                "id": memory.id.to_string(),
                "class": memory.class,
                "date": memory.at.local_date(offset),
                "text": memory.text,
            });
            if let Some(who) = &memory.who {
                item["who"] = serde_json::Value::String(who.clone());
            }
            if memory.retired {
                item["forgotten"] = serde_json::Value::Bool(true);
            }
            item
        })
        .collect();
    let turns: Vec<serde_json::Value> = found
        .turns
        .iter()
        .map(|turn| {
            serde_json::json!({
                "date": turn.at.local_date(offset),
                "session": turn.session.as_str(),
                "text": turn.text,
            })
        })
        .collect();
    let cards: Vec<serde_json::Value> = found
        .cards
        .iter()
        .map(|card| {
            serde_json::json!({
                "who": card.who,
                "names": card.names,
                "date": card.at.local_date(offset),
                "text": card.text,
            })
        })
        .collect();
    let mut shown = serde_json::json!({ "memories": memories, "turns": turns });
    // 没卡的不写这一格（施工 E-3）：以前的样子不变。
    if !cards.is_empty() {
        shown["cards"] = serde_json::Value::Array(cards);
    }
    shown
}

/// 卡上的是谁：有叫法的接在后面，同认出来那一块（`card/person.txt`）。
fn who(card: &FoundCard) -> String {
    match card.names.is_empty() {
        true => card.who.clone(),
        false => format!("{} ({})", card.who, card.names.join(", ")),
    }
}

/// 一段对话写成一行：换行（人的话和她的回答之间的空行）换成 ` / `，截到 [`TURN_CHARS`] 个字，截了的末尾接 `…`。
fn flat(text: &str) -> String {
    let joined = text
        .split('\n')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    match joined.char_indices().nth(TURN_CHARS) {
        Some((cut, _)) => format!("{}…", &joined[..cut]),
        None => joined,
    }
}

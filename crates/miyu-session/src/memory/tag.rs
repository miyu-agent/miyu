//! 写关键词（施工 R-15 下，`docs/blueprint/memory.md` 第二条）：另发一次只写关键词的请求，抽取、整理原来那一次一个字都不改
//! （2026-10-19 量：塞进那一次，抽取多记五六成、整理改和作废的少一半）。抽取记下以后写这几条；整理落完以后顺带补这一间还
//! 没有的，最多 [`BATCH`] 条（项目主人定：旧的整理时慢慢补）。
//!
//! - 照 `memory.organizer` 发（同抽取、合并），用途 `memory` 记账，遮 key。
//! - 落的时候那一条还算数、听众合、还没有关键词的才写（人改过的、她当场给了的不盖）；换了字的向量由调的一方接着补。
//! - 发不出去、交回读不成的记一行 `WARN memory tagging failed`，不重试：整理时还挑得到。

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::AccountId;
use miyu_kernel::request::Message;
use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_recall::redact::redact;
use miyu_recall::tag::keywords;
use miyu_recall::{Entry, MemoryId};
use miyu_tool::load::{self, LoadError, say};

use super::extract::secrets;
use super::merge::MergeJob;
use super::{Extraction, Keeper};
use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::config::TurnConfig;
use crate::route::Ask;
use crate::settings::MemorySettings;

/// 整理时一次最多补几条：真数据上 20 条一批照样条条写上（2026-10-19 量），一次多花几千个 token。
pub(crate) const BATCH: usize = 20;

/// 用途：同抽取、合并（`memory`）。
const PURPOSE: &str = "memory";

const PACKAGE: &str = "memory";

/// 写关键词给模型看的字（`resources/software/memory/tag/`，登记簿）：指令，一条一行照 `memory_search/memory.txt`。
#[derive(Debug, Clone)]
pub struct TagTexts {
    instruction: Template,
    line: Template,
}

impl TagTexts {
    /// 从资源目录 `resources` 读：`tag/instruction.txt`，一行照 `memory_search/memory.txt`（`{id}`、`{class}`、`{date}`、`{text}`）。
    ///
    /// # Errors
    ///
    /// 哪一份读不出来、写法不对、要了别的字段。
    pub fn load(resources: &Path) -> Result<TagTexts, LoadError> {
        Ok(TagTexts {
            instruction: load::text(resources, PACKAGE, "tag", "instruction", &[])?,
            line: load::text(
                resources,
                PACKAGE,
                "memory_search",
                "memory",
                &["id", "class", "date", "text"],
            )?,
        })
    }
}

/// 写一次要的几样：哪一间、抽取那一次的（字、一次性入口、key 的写法）、这一轮的配置、记在谁的账上、会话的时区。
pub(crate) struct Tagging {
    pub(crate) keeper: Keeper,
    pub(crate) extraction: Extraction,
    pub(crate) config: TurnConfig,
    pub(crate) owner: AccountId,
    pub(crate) offset: UtcOffset,
}

impl Tagging {
    /// 给 `entries` 这几条写关键词，交回写上了几条；一条都没有的、没交写关键词的字的不发。出错的记一行，交回 0。补向量是调的
    /// 一方的事。
    pub(crate) async fn run(&self, entries: Vec<Entry>) -> u32 {
        let Some(texts) = &self.extraction.tag else {
            return 0;
        };
        if entries.is_empty() {
            return 0;
        }
        let started = Instant::now();
        let room = self.keeper.room().to_string();
        let rows: Vec<String> = entries
            .iter()
            .map(|entry| {
                say(
                    &texts.line,
                    &[
                        ("id", &entry.id.to_string()),
                        (
                            "class",
                            &miyu_recall::shown_class(&entry.class, entry.who.as_deref()),
                        ),
                        ("date", &entry.at.local_date(self.offset)),
                        ("text", &entry.text.replace('\n', " ")),
                    ],
                )
            })
            .collect();
        let given: BTreeSet<MemoryId> = entries.iter().map(|entry| entry.id).collect();
        let secrets = secrets(&self.config);
        let shapes = &self.extraction.shapes;
        let text = format!(
            "{}\n{}",
            say(&texts.instruction, &[]).trim_end(),
            rows.join("\n")
        );
        let ask = Ask {
            model: MemorySettings::from(&self.config.resolved.values()).organizer,
            purpose: PURPOSE.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                blocks: vec![Block::Text(Text {
                    text: redact(&text, &secrets, shapes),
                })],
            }],
            max_tokens: None,
            owner: self.owner.clone(),
        };
        let answered = self
            .extraction
            .ask
            .call(&self.config, &self.extraction.blobs, ask)
            .await
            .map_err(|unanswered| unanswered.reason().to_string())
            .and_then(|answer| keywords(&answer.text, &given));
        let found = match answered {
            Ok(found) => found,
            Err(why) => {
                tracing::warn!(target: TARGET, room = room.as_str(), given = given.len(), error = why.as_str(), "memory tagging failed");
                return 0;
            }
        };
        let found: Vec<(MemoryId, String)> = found
            .into_iter()
            .map(|(id, keywords)| (id, redact(&keywords, &secrets, shapes)))
            .collect();
        let keeper = self.keeper.clone();
        let count = match blocking(move || keeper.record_tags(wall_now(), found)).await {
            Ok(count) => count,
            Err(refused) => {
                tracing::warn!(target: TARGET, room = room.as_str(), error = ?refused, "memory tagging not recorded");
                return 0;
            }
        };
        let took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(target: TARGET, room = room.as_str(), count, took_ms, "memory tagged");
        count
    }
}

impl MergeJob {
    /// 补这一间还没有关键词的，最多 [`BATCH`] 条：`given` 里的在前，都照新的在前。交回写上了几条。
    pub(super) async fn tag(&self, given: &BTreeSet<MemoryId>) -> u32 {
        let keeper = self.keeper.clone();
        let first: Vec<MemoryId> = given.iter().rev().copied().collect();
        let entries = match blocking(move || keeper.untagged(&first, BATCH)).await {
            Ok(entries) => entries,
            Err(refused) => {
                tracing::warn!(target: TARGET, room = %self.keeper.room(), error = ?refused, "memory tagging not read");
                return 0;
            }
        };
        let tagging = Tagging {
            keeper: self.keeper.clone(),
            extraction: self.extraction.clone(),
            config: Arc::clone(&self.config),
            owner: self.owner.clone(),
            offset: self.offset,
        };
        tagging.run(entries).await
    }
}

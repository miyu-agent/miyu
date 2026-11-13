//! 派出去抽的那一件活（施工 R-6 上，`docs/blueprint/memory.md` 第六条第 6 到 8 款；施工 R-7 补从 `extract.rs` 挪出来）：照
//! 定好的办法跳过、发、读、遮、记，记下以后看一眼够不够合并。

use miyu_models::side::ORGANIZE;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::oneshot;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::request::Message;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::Skipped;
use miyu_recall::extract::candidates;
use miyu_recall::redact::redact;
use miyu_tool::load::say;
use miyu_tool::{Line, TEXT_CHARS};

use super::{DROPPED, Extraction, Extractor, PURPOSE, Plan, TRIES, secrets};
use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::config::TurnConfig;
use crate::memory::landed::Landed;
use crate::memory::merge::{Hush, MergeJob};
use crate::memory::{Dreamed, Keeper, NotDreamed, Stamp};
use crate::route::Ask;

/// 派出去抽的一次。
pub(crate) struct Job {
    /// 记在哪一间、听众是谁。
    pub(crate) keeper: Keeper,
    /// 字、key 的写法、一次性入口。
    pub(crate) extraction: Extraction,
    /// 这一轮的配置：照它取 key、发。
    pub(crate) config: TurnConfig,
    /// 哪个会话。
    pub(crate) session: SessionId,
    /// 这个会话这一刻的引用（施工 Q-2）：整理记忆的档位池空了、用不了时退到它；没有的照 `models.chat`。
    pub(crate) fallback: Option<String>,
    /// 从第几条以后抽（上次抽到的；没抽过的是 0 那种，照 `Seq` 最小的）。
    pub(crate) after: Seq,
    /// 这一回的状态：走完了清掉。
    pub(crate) extractor: Arc<Extractor>,
    /// 放不下、还剩几轮的，抽完了照它马上再起一次（不用再等闲）。
    pub(crate) again: Box<dyn Fn() + Send + Sync>,
    /// 会话的时区：合并的日期照它写（施工 R-7 上）。
    pub(crate) offset: UtcOffset,
    /// 人叫她现在就整理（施工 R-7 补，`/dream`）：抽完了不看够不够、现在就合，合完把几样数交回这一头。没叫的照定时的看一眼。
    pub(crate) dream: Option<oneshot::Sender<Result<Dreamed, NotDreamed>>>,
    /// 人叫的整理听着的叫停（施工 R-22）；定时的听不见。
    pub(crate) hush: Hush,
}

impl Job {
    /// 照 `plan` 走一次：跳过的只记抽到了哪；要发的发、读、遮、记。之后：人叫了现在就整理的现在就合、把结果交回去；没叫的
    /// 这一次真记下了至少一条才看一眼够不够合并（施工 E-9：抽到 0 条、整段跳过的不看；没有轮询，也不另上闹钟）。
    pub(crate) async fn run(mut self, plan: Plan) {
        let saved = match plan {
            Plan::Wait => {
                self.extractor.finish(self.after, true);
                0
            }
            Plan::Skip { upto } => {
                self.mark(
                    upto,
                    (Vec::new(), BTreeMap::new()),
                    Some(Skipped::Remembered),
                )
                .await;
                self.extractor.finish(self.after, true);
                0
            }
            Plan::Ask {
                upto,
                text,
                turns,
                more,
                since,
                names,
                me,
                places,
            } => {
                let recorded = match self.dropped(text, since).await {
                    Some(text) => self.ask(upto, text, turns, (&names, me), places).await,
                    None => None,
                };
                if recorded.is_some() && more {
                    (self.again)();
                }
                recorded.unwrap_or(0)
            }
        };
        match self.dream.take() {
            Some(reply) => {
                let dreamed = self.merge_job().now().await;
                // 回应以前放下听的那一头：回了以后再叫停的说没在整理（施工 R-22）。
                drop(std::mem::take(&mut self.hush));
                if reply.send(dreamed).is_err() {
                    tracing::debug!(target: TARGET, session = %self.session, "dream answer not taken");
                }
            }
            None if saved > 0 => self.merge_job().start(),
            None => {}
        }
    }

    /// 这一段 `text` 后面接上第一句话的时刻 `since` 以后忘掉的几条（施工 R-6 修，`memory.md` 第六条第 4 款）：一条都没有的
    /// 原样交回。记忆日志读不了的这一次放下（不算这一段失败），交回没有。
    async fn dropped(&self, text: String, since: Timestamp) -> Option<String> {
        let keeper = self.keeper.clone();
        let dropped = blocking(move || keeper.dropped_since(since, DROPPED)).await;
        let dropped = match dropped {
            Ok(dropped) => dropped,
            Err(refused) => {
                tracing::warn!(target: TARGET, session = %self.session, error = ?refused, "memory extraction failed");
                self.extractor.abandon();
                return None;
            }
        };
        if dropped.is_empty() {
            return Some(text);
        }
        let texts = &self.extraction.texts;
        let lines: Vec<String> = dropped
            .iter()
            .map(|entry| {
                texts
                    .lines
                    .say(&Line {
                        id: &entry.id.to_string(),
                        class: &miyu_recall::shown_class(&entry.class, entry.who.as_deref()),
                        date: &entry.at.local_date(self.offset),
                        text: &entry.text.replace('\n', " "),
                        venue: entry.venue.as_ref(),
                        place: entry.place.as_deref(),
                        retired: false,
                    })
                    .trim_end()
                    .to_string()
            })
            .collect();
        Some(format!(
            "{text}\n\n{}\n{}",
            say(&texts.dropped, &[]).trim_end(),
            lines.join("\n")
        ))
    }

    /// 发、读、遮、记；交回记下了几条（抽到哪记成了的，0 条也算记成了），没记成的交回没有。
    async fn ask(
        &self,
        upto: Seq,
        text: String,
        turns: BTreeSet<u64>,
        (names, here): (&BTreeMap<String, Option<String>>, BTreeSet<String>),
        places: BTreeMap<u64, String>,
    ) -> Option<usize> {
        let started = Instant::now();
        let secrets = secrets(&self.config);
        let shapes = self.extraction.shapes.clone();
        tracing::info!(target: TARGET, session = %self.session, turns = turns.len(), "memory extraction started");
        let ask = Ask {
            model: None,
            purpose: PURPOSE.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                blocks: vec![Block::Text(Text {
                    text: redact(&text, &secrets, &shapes),
                })],
            }],
            max_tokens: None,
            venue: None,
        };
        let answered = self
            .extraction
            .ask
            .call_side(
                &self.config,
                &self.extraction.blobs,
                ask,
                &ORGANIZE,
                self.fallback.clone(),
            )
            .await
            .map_err(|unanswered| unanswered.reason().to_string())
            .and_then(|answer| candidates(&answer.text, &turns, TEXT_CHARS));
        match answered {
            Ok(mut found) => {
                // 她：人格的名字，加这一段里她在场所里被叫过的（施工 E-1 修三）。
                let mut me = self.keeper.her();
                me.extend(here);
                super::place_people(&mut found, names, &me);
                for candidate in &mut found {
                    candidate.text = redact(&candidate.text, &secrets, &shapes);
                }
                let recorded = self.mark(upto, (found, places), None).await;
                if let Some(saved) = &recorded {
                    let took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let count = saved.len();
                    tracing::info!(target: TARGET, session = %self.session, count, took_ms, "memory extracted");
                    if !saved.is_empty() {
                        // 接着给这几条写关键词（施工 R-15 下），再补向量：写上了的照新的字补，只补一遍。
                        self.landed().settle(saved).await;
                    }
                }
                self.extractor.finish(self.after, true);
                recorded.map(|saved| saved.len())
            }
            Err(why) => {
                let tries = self.extractor.finish(self.after, false);
                tracing::warn!(target: TARGET, session = %self.session, tries, error = why.as_str(), "memory extraction failed");
                if tries >= TRIES {
                    self.mark(upto, (Vec::new(), BTreeMap::new()), Some(Skipped::Failed))
                        .await;
                }
                None
            }
        }
    }

    /// 记下了几条以后的那几样（施工 MI-1 挪进 `landed.rs`，导入也用）：写关键词、补向量。
    fn landed(&self) -> Landed {
        Landed {
            keeper: self.keeper.clone(),
            extraction: self.extraction.clone(),
            config: Arc::clone(&self.config),
            offset: self.offset,
        }
    }

    /// 照这一次的几样合并（施工 R-7 上，`merge.rs`）：定时的看一眼够不够、在后台合，人叫的现在就合。照主人那边合（施工
    /// R-13 上）：一个大脑一份摘要，场所会话里抽完起的也照全部合。
    fn merge_job(&self) -> MergeJob {
        MergeJob {
            keeper: self.keeper.as_owner(),
            extraction: self.extraction.clone(),
            config: Arc::clone(&self.config),
            offset: self.offset,
            hush: self.hush.clone(),
            from: Some(self.session.clone()),
        }
    }

    /// 记下几条（`found`，每一轮的群名 `places`）、记抽到了第 `upto` 条，交回记下的编号；写不进的记一行，交回没有。
    async fn mark(
        &self,
        upto: Seq,
        (found, places): (Vec<miyu_recall::extract::Candidate>, BTreeMap<u64, String>),
        skipped: Option<Skipped>,
    ) -> Option<Vec<miyu_recall::MemoryId>> {
        // 人生结束了的（施工 LF-4 上）：落地前再看一次，整批丢掉。
        if self.extractor.closed() {
            tracing::info!(target: TARGET, session = %self.session, "memory extraction dropped life_ended=true");
            return None;
        }
        let (keeper, session) = (self.keeper.clone(), self.session.clone());
        let closed = Arc::clone(&self.extractor);
        let recorded = blocking(move || {
            let recorded =
                keeper.record_extraction(wall_now(), &session, upto, (found, &places), skipped);
            // 看过以后、写下以前结束了的：结束那边的清空可能已经清过了，这里再清一次这个会话的，不留下（`life.md`「结束」）。
            if closed.closed() {
                let stamp = Stamp {
                    at: wall_now(),
                    by: By::Kernel,
                    cause: None,
                };
                if let Err(error) = keeper.clear(stamp, Some(session.clone())) {
                    tracing::warn!(target: TARGET, session = %session, error = ?error, "memory extraction not cleared");
                }
            }
            recorded
        })
        .await;
        recorded
            .inspect_err(|error| {
                tracing::warn!(target: TARGET, session = %self.session, error = ?error, "memory extraction not recorded");
            })
            .ok()
    }
}

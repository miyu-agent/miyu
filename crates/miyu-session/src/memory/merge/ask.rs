//! 发一批、读、遮、落（施工 R-7 上；D-2 起跑在整理会话里，从 `merge.rs` 拆出来）。
//!
//! - 接了会话表的（[`crate::Memory::dream_sessions`]）：这一批是整理会话的一轮（[`DreamSessions::batch`]），思考、用量、用时都
//!   在会话里；交回回答的原文，照旧读、核对、落。叫停就是打断那一轮（设计 41 DS4）。
//! - 没接的（测试里没接会话表的核心）：照旧走一次性入口。
//! - 她正在写什么（施工 R-21）：两路都边收边推，经同一个 [`MergeJob::watcher`]（整理会话那一路的正文增量 D-1 下起由 `batch`
//!   交）。

use miyu_models::side::ORGANIZE;
use std::sync::Arc;
use std::time::Instant;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{CommandId, Seq, SessionId};
use miyu_kernel::request::Message;
use miyu_recall::merge::{Seen, Watch, cards, decisions, kept_names};
use miyu_recall::redact::{KeyShapes, redact};
use miyu_tool::TEXT_CHARS;

use super::{Asked, Dreamed, MergeJob, PURPOSE, SUMMARY_CHARS, TRIES};
use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::memory::extract::secrets;
use crate::memory::{DreamBatch, DreamSessions, Dreaming, Using};
use crate::route::{Ask, Watched};

/// 这一趟的整理会话（设计 41）：会话表、会话编号。
pub(super) struct Trip {
    pub(super) sessions: Arc<dyn DreamSessions>,
    pub(super) session: SessionId,
}

/// 一批问完了：答了的原文，或者没答成的为什么（数一次失败）；人叫停、会话这一路断了的另说。
enum Asking {
    Answered(String),
    Failed(String),
    Stopped,
}

impl MergeJob {
    /// 发第 `n` 批（从 1 数）、读、遮、记；交回这一次合了多少。`trip` 是这一趟的整理会话，没有的走一次性入口。
    ///
    /// # Errors
    ///
    /// 发不出去、交回读不成、记忆日志写不进、整理会话那一路断了（核心在停、会话没了）：英文的一句为什么，当场记了 `WARN`。
    pub(super) async fn ask(
        &self,
        asked: &Asked,
        trip: Option<&Trip>,
        n: u32,
    ) -> Result<Dreamed, String> {
        let started = Instant::now();
        let (room_kind, room_id) = (self.keeper.room().kind(), self.keeper.room_id());
        let secrets = secrets(&self.config);
        let shapes = &self.extraction.shapes;
        tracing::info!(target: TARGET, room_kind, room_id = room_id.as_str(), given = asked.given.len(), fresh = asked.fresh, "memory merge started");
        let text = redact(&asked.text, &secrets, shapes);
        let asking = match trip {
            Some(trip) => self.in_session(asked, trip, n, text).await?,
            None => self.at_once(asked, text).await,
        };
        let answered = match asking {
            Asking::Answered(text) => decisions(&text, &asked.given, TEXT_CHARS, SUMMARY_CHARS)
                .and_then(|decided| Ok((decided, cards(&text, &asked.people)?))),
            Asking::Failed(why) => Err(why),
            Asking::Stopped => return self.hushed(asked.upto).await,
        };
        let merges = &self.keeper.memory.merges;
        let given = u32::try_from(asked.given.len()).unwrap_or(u32::MAX);
        match answered {
            Ok((mut decided, mut drafted)) => {
                for revised in &mut decided.revised {
                    revised.text = redact(&revised.text, &secrets, shapes);
                }
                for retire in &mut decided.retired {
                    retire.why = redact(&retire.why, &secrets, shapes);
                }
                if let Some(summary) = &mut decided.summary {
                    *summary = redact(summary, &secrets, shapes);
                }
                // 人卡（施工 E-1 下）：遮过 key。叫法核心认（施工 E-1 修二）：他在出处里真用过的、认出来的在前，交回的只留出处
                // 里见过的。
                for card in &mut drafted {
                    card.text = redact(&card.text, &secrets, shapes);
                    let known = asked.known.names_of(&card.who);
                    let people = &asked.known;
                    card.names = kept_names(&card.who, &known, &card.names, |name| {
                        people.seen(&card.who, name)
                    })
                    .iter()
                    .map(|name| redact(name, &secrets, shapes))
                    .collect();
                }
                let summary = decided.summary.is_some();
                let (keeper, upto, swept) = (self.keeper.clone(), asked.upto, asked.swept.clone());
                let recorded = blocking(move || {
                    keeper.record_merge(
                        wall_now(),
                        &decided,
                        &drafted,
                        (upto, given),
                        swept.as_ref(),
                    )
                })
                .await;
                merges.record(self.keeper.room(), true);
                match recorded {
                    Ok((revised, retired)) => {
                        let took_ms =
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                        tracing::info!(target: TARGET, room_kind, room_id = room_id.as_str(), revised, retired, took_ms, "memory merged");
                        // 顺带补这一间还没有关键词的（施工 R-15 下，项目主人定旧的整理时慢慢补）：这一次交进去的在前。
                        let tagged = tokio::select! {
                            tagged = self.tag(&asked.given) => tagged,
                            () = self.hush.wait() => 0,
                        };
                        if revised > 0 || tagged > 0 {
                            // 改出来的、换了关键词的几条在后台补向量（施工 R-5 五补）。
                            let keeper = self.keeper.clone();
                            let using = Using {
                                config: Arc::clone(&self.config),
                                owner: self.owner.clone(),
                            };
                            blocking(move || keeper.fill(&using)).await;
                        }
                        Ok(Dreamed {
                            given,
                            revised,
                            retired,
                            summary,
                            ..Dreamed::default()
                        })
                    }
                    Err(refused) => {
                        tracing::warn!(target: TARGET, room_kind, room_id = room_id.as_str(), error = ?refused, "memory merge not recorded");
                        Err(format!("{refused:?}"))
                    }
                }
            }
            Err(why) => {
                let tries = merges.record(self.keeper.room(), false);
                tracing::warn!(target: TARGET, room_kind, room_id = room_id.as_str(), tries, error = why.as_str(), "memory merge failed");
                // 每一次没成都记一行（施工 R-17），连着第三次的另记跳过这一批。
                let (keeper, upto) = (self.keeper.clone(), asked.upto);
                let said = redact(&why, &secrets, shapes);
                let failed = blocking(move || {
                    keeper.record_merge_failed(
                        wall_now(),
                        upto,
                        given,
                        &said,
                        tries,
                        tries >= TRIES,
                    )
                })
                .await;
                if let Err(refused) = failed {
                    tracing::warn!(target: TARGET, room_kind, room_id = room_id.as_str(), error = ?refused, "memory merge not recorded");
                }
                Err(why)
            }
        }
    }

    /// 经一次性入口问（没接会话表的核心）：边收边推她正在写什么；人叫停了这一批放弃，请求跟着丢掉（施工 R-22）。
    async fn at_once(&self, asked: &Asked, text: String) -> Asking {
        let ask = Ask {
            model: None,
            purpose: PURPOSE.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                blocks: vec![Block::Text(Text { text })],
            }],
            max_tokens: None,
            owner: self.owner.clone(),
            venue: None,
        };
        let mut tell = self.watcher(asked);
        let calling = self.extraction.ask.call_side_watching(
            &self.config,
            &self.extraction.blobs,
            ask,
            (&ORGANIZE, None),
            &mut tell,
        );
        tokio::select! {
            answered = calling => match answered {
                Ok(answer) => Asking::Answered(answer.text),
                Err(unanswered) => Asking::Failed(unanswered.reason().to_string()),
            },
            () = self.hush.wait() => Asking::Stopped,
        }
    }

    /// 在整理会话里问第 `n` 批（设计 41）：一批一轮，命令编号 `<会话>/dream/<n>`。人叫停了打断那一轮（`<会话>/dream/<n>/stop`）、
    /// 等它收尾（DS4）；会话自己被打断了的同样算叫停。
    ///
    /// # Errors
    ///
    /// 会话那一路断了（核心在停、会话没了、写不进）：不数失败，这一趟停，下次再合。
    async fn in_session(
        &self,
        asked: &Asked,
        trip: &Trip,
        n: u32,
        text: String,
    ) -> Result<Asking, String> {
        let id = CommandId::parse(&format!("{}/dream/{n}", trip.session))
            .map_err(|error| error.to_string())?;
        let mut tell = self.watcher(asked);
        let mut batching = trip
            .sessions
            .batch(trip.session.clone(), id, text, &mut tell);
        let ran = tokio::select! {
            ran = &mut batching => ran,
            () = self.hush.wait() => {
                // 叫停另用一个编号：和这一批同一个的会当成重发。
                let stop = CommandId::parse(&format!("{}/dream/{n}/stop", trip.session))
                    .map_err(|error| error.to_string())?;
                if let Err(why) = trip.sessions.stop(trip.session.clone(), stop).await {
                    tracing::warn!(target: TARGET, room_kind = self.keeper.room().kind(), room_id = self.keeper.room_id().as_str(), error = why.as_str(), "dream session not stopped");
                }
                // 等那一轮收尾：下一趟不会和它撞上。答了的也不要：叫停了。
                drop(batching.await);
                return Ok(Asking::Stopped);
            }
        };
        match ran {
            Ok(DreamBatch::Answered(text)) => Ok(Asking::Answered(text)),
            Ok(DreamBatch::Failed(why)) => Ok(Asking::Failed(why)),
            Ok(DreamBatch::Stopped) => Ok(Asking::Stopped),
            Err(why) => {
                tracing::warn!(target: TARGET, room_kind = self.keeper.room().kind(), room_id = self.keeper.room_id().as_str(), error = why.as_str(), "dream session gone");
                Err(why)
            }
        }
    }

    /// 她正在写什么（施工 R-21）：边收边读，写完一条推一条，遮过 key，不落盘。两路共用这一个。
    fn watcher<'a>(&'a self, asked: &Asked) -> impl FnMut(Watched<'_>) + Send + 'a {
        let secrets = secrets(&self.config);
        let mut watch = Watch::new(asked.given.clone(), TEXT_CHARS);
        let upto = asked.upto;
        move |watched: Watched<'_>| {
            let seen = match watched {
                Watched::Text(text) => watch.feed(text),
                Watched::Restart => {
                    watch.restart();
                    return;
                }
            };
            for seen in seen {
                let seen = hide(seen, &secrets, &self.extraction.shapes);
                let dreaming = Dreaming { upto, seen };
                self.keeper.memory.dreams.tell(self.keeper.room(), dreaming);
            }
        }
    }

    /// 人叫停了这一批（施工 R-22）：记一条 `stopped`，交回什么都没合。写不进的记一行，照样算停了。
    pub(super) async fn hushed(&self, upto: Seq) -> Result<Dreamed, String> {
        let keeper = self.keeper.clone();
        if let Err(refused) = blocking(move || keeper.record_merge_stopped(wall_now(), upto)).await
        {
            tracing::warn!(target: TARGET, room_kind = self.keeper.room().kind(), room_id = self.keeper.room_id().as_str(), error = ?refused, "memory merge not recorded");
        }
        Ok(Dreamed {
            stopped: true,
            ..Dreamed::default()
        })
    }
}

/// 推以前遮过 key（同落的）。
fn hide(seen: Seen, secrets: &[String], shapes: &KeyShapes) -> Seen {
    match seen {
        Seen::Revised(mut revised) => {
            revised.text = redact(&revised.text, secrets, shapes);
            Seen::Revised(revised)
        }
        Seen::Retired(mut retire) => {
            retire.why = redact(&retire.why, secrets, shapes);
            Seen::Retired(retire)
        }
        summary @ Seen::Summary { .. } => summary,
    }
}

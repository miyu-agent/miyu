//! 合并（施工 R-7 上，`docs/blueprint/memory.md` 第七条）：抽取只往里加，隔一段时间、攒够几个会话，把新记的和相关的旧记忆放在
//! 一起理一遍：重复的、被推翻的作废，相对的日期改成绝对的，写一段新的摘要。
//!
//! - **什么时候**：抽取记下以后看一眼（`extract.rs` 的 `Job`）：上次合并以后有新记的（现在算数、听众合、出处活着，不算合并
//!   自己改出来的）；那以后抽过的会话够 `memory.merge_sessions`；离上次合并过了 `memory.merge_every`（没合并过的不看时间）。
//!   一间一把锁（`Merges`，`merge/merges.rs`），在后台合，不挡会话。
//! - **交什么**：一条 user，指令在前，后面是现在的摘要、上次合并以后新记的（旧的在前）、每条新记的照关键词和意思搜出来的相关
//!   旧记忆（每条最多 [`RELATED`] 条，去重）。一条一行，同 `memory_search` 的写法。最多 `ROOM` 字节（`extract.rs`）：放不下的合完马上接着
//!   合剩下的。发出去以前遮 key。
//! - **交回、怎么落**：读法在 `miyu_recall::merge`；改的指着旧的（类、出处、听众照旧）、作废的带为什么、摘要、合到哪的记号，
//!   都由记忆模块记。落的时候还算数、听众合的才落。
//! - **现在就整理**（施工 R-7 补，`/dream`、`memory.dream`）：不看会话数、间隔，现在就合到没有剩下的，交回几样数
//!   （[`Dreamed`]）；同一间正在合的交回忙。会话的 `/stop` 连它叫的这一趟一起停（施工 R-22，`merge/hush.rs`）：合完了的批留着，
//!   正在合的那一批放弃（记一条 `stopped`）、下次再合。
//! - **出错**：不动真相，下次抽完再来；同一间连着 [`TRIES`] 次不成的，记一条失败的记号、跳过这一批（成了一次、跳过一次都重新
//!   数：这一批从哪起只在这两种时候变）。

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{AccountId, Seq};
use miyu_kernel::request::Message;
use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_recall::merge::{Seen, Watch, decisions};
use miyu_recall::redact::redact;
use miyu_recall::{Entry, MemoryId, Swept};
use miyu_tool::TEXT_CHARS;
use miyu_tool::load::{self, LoadError};

use super::extract::secrets;
use super::{Dreaming, Extraction, Keeper, Using};
use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::config::TurnConfig;
use crate::route::{Ask, Watched};
use crate::settings::MemorySettings;

mod compose;
mod hush;
mod land;
mod merges;

pub(crate) use hush::{Hush, Hushes};
pub(crate) use merges::Merges;

/// 每条新记的最多带几条相关的旧记忆。
pub(crate) const RELATED: usize = 3;

/// 摘要最多几个字：常驻的那一块 3000 字节里，摘要占一半上下（中文一个字三个字节），留地方给之后新记的（R-7 下）。
pub(crate) const SUMMARY_CHARS: usize = 600;

/// 同一批连着合不成几次就跳过。
const TRIES: u32 = 3;

/// 用途：记运行日志、记账（和抽取的同一个）。
const PURPOSE: &str = "memory";

/// 记忆这个软件包在资源目录里的名字（和 `extract.rs` 的同一个）。
const PACKAGE: &str = "memory";

/// 合并给模型看的字（`resources/software/memory/merge/`，登记簿）：指令、现在的摘要、还没有摘要、两块的标题，一条一行照
/// `memory_search/memory.txt`。
#[derive(Debug, Clone)]
pub struct MergeTexts {
    instruction: Template,
    summary: Template,
    none: Template,
    new: Template,
    related: Template,
    /// 顺带重看的旧记忆那一块的标题（施工 R-19）。
    sweep: Template,
    line: Template,
    unnamed: Template,
}

impl MergeTexts {
    /// 从资源目录 `resources` 读：`merge/instruction.txt`（`{owner}`）、`summary.txt`（`{text}`）、`none.txt`、`new.txt`、
    /// `related.txt`、`sweep.txt`（施工 R-19），一行照 `memory_search/memory.txt`（`{id}`、`{class}`、`{date}`、`{text}`），没有称呼时主人写的那一句
    /// `common/owner-unnamed.txt`（施工 R-6 再修）。
    ///
    /// # Errors
    ///
    /// 哪一份读不出来、写法不对、要了别的字段。
    pub fn load(resources: &Path) -> Result<MergeTexts, LoadError> {
        let text =
            |name: &str, fields: &[&str]| load::text(resources, PACKAGE, "merge", name, fields);
        Ok(MergeTexts {
            instruction: text("instruction", &["owner"])?,
            summary: text("summary", &["text"])?,
            none: text("none", &[])?,
            new: text("new", &[])?,
            related: text("related", &[])?,
            sweep: text("sweep", &[])?,
            line: load::text(
                resources,
                PACKAGE,
                "memory_search",
                "memory",
                &["id", "class", "date", "text"],
            )?,
            unnamed: load::text(resources, PACKAGE, "common", "owner-unnamed", &[])?,
        })
    }
}

/// 是合并自己改出来的（`by` 是记忆模块、写了 `replaces`）：合并不当它是新记的，常驻的那一块有摘要时不列它（施工 R-7 下）。
pub(super) fn made_by_merge(entry: &Entry) -> bool {
    entry.replaces.is_some()
        && matches!(&entry.by, miyu_kernel::origin::By::Module(module) if module.id.as_str() == super::MODULE)
}

/// 现在就整理一次的结果（施工 R-7 补，`memory.md` 第七条第 9 款）：交进去几条（新记的加相关的，几次合的加起来）、改了几条、
/// 作废几条、摘要换没换。没有要整理的四样都是零。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dreamed {
    /// 交进去几条。
    pub given: u32,
    /// 改了几条。
    pub revised: u32,
    /// 作废几条。
    pub retired: u32,
    /// 写了新的摘要。
    pub summary: bool,
    /// 人叫停了（施工 R-22）：前面几样是叫停以前合完了的。
    pub stopped: bool,
}

/// 没整理成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotDreamed {
    /// 这一间正在合（定时的那一次，或者别的人先按了）。
    Busy,
    /// 这里没有记忆可整理：人格记忆没装、核心没交整理要的几样、这个会话的记忆关着。
    Off,
    /// 请求模型没成、交回的读不成、记忆日志写不进：为什么（英文的一句，运行日志里另有那一行）。真相没动。
    Failed(String),
}

impl Keeper {
    /// 现在就整理这一间（施工 R-7 补，`memory.md` 第七条第 9 款）：不看会话数、间隔，照 `config`（`memory.organizer` 和 key）
    /// 发，记在 `owner` 的账上，日期照时区 `offset` 写。不经会话的那一路（协议带 `persona` 的 `memory.dream`）；会话里的先抽
    /// 再调它（`Handle::dream`）。
    ///
    /// # Errors
    ///
    /// 人格记忆没装、核心没交整理要的几样（[`NotDreamed::Off`]）；这一间正在合（[`NotDreamed::Busy`]）；有一次没成
    /// （[`NotDreamed::Failed`]）。
    pub async fn dream(
        &self,
        config: TurnConfig,
        owner: AccountId,
        offset: UtcOffset,
    ) -> Result<Dreamed, NotDreamed> {
        let hush = self.hush_for(Hush::default());
        self.dream_until(config, owner, offset, hush).await
    }

    /// 同 [`Keeper::dream`]，`hush` 叫停了就停（施工 R-22，会话里的 `/dream`）。
    pub(crate) async fn dream_until(
        &self,
        config: TurnConfig,
        owner: AccountId,
        offset: UtcOffset,
        hush: Hush,
    ) -> Result<Dreamed, NotDreamed> {
        let Some(extraction) = self.extraction() else {
            return Err(NotDreamed::Off);
        };
        MergeJob {
            keeper: self.as_owner(),
            extraction,
            config,
            owner,
            offset,
            hush,
        }
        .now()
        .await
    }
}

/// 合并要的几样：抽取那一次的（字、一次性入口、key 的写法）、这一轮的配置、记在谁的账上、会话的时区。
pub(crate) struct MergeJob {
    /// 哪一间、听众是谁。
    pub(crate) keeper: Keeper,
    /// 字、一次性入口、key 的写法。
    pub(crate) extraction: Extraction,
    /// 这一轮的配置：照它取 key、发、读 `memory.merge_*`。
    pub(crate) config: TurnConfig,
    /// 记在谁的账上：会话的属主。
    pub(crate) owner: AccountId,
    /// 日期照会话的时区写。
    pub(crate) offset: UtcOffset,
    /// 人叫停了就停（施工 R-22）；定时的听不见。
    pub(crate) hush: Hush,
}

/// 一批：现在的摘要、上次合并以后新记的（旧的在前）。
struct Batch {
    summary: Option<String>,
    fresh: Vec<Entry>,
    /// 一趟的头一批顺带重看的旧记忆（施工 R-19），照编号；别的批是空的。
    sweep: Vec<Entry>,
    /// 上次合到的那一条：只重看的一批照它记合到哪。
    after: Option<Seq>,
}

/// 一批带不带轮到的旧记忆（施工 R-19）。
enum Sweep {
    /// 照底账挑：一趟的头一批。
    Pick,
    /// 前一批让新记的挤掉了，带上这几条。
    Carry(Vec<Entry>),
    /// 这一趟看过了、或者没有可看的。
    Done,
}

/// 交出去的一次：请求的字、交进去的编号、合到第几条、其中新记的几条、顺带重看到哪（施工 R-19）。
struct Asked {
    text: String,
    given: BTreeSet<MemoryId>,
    upto: Seq,
    fresh: u32,
    swept: Option<Swept>,
}

impl MergeJob {
    /// 抽取记下以后调：够了的在后台合（`memory.md` 第七条第 1 款）。同一间已经在合的、不够的什么都不做。
    pub(crate) fn start(self) {
        tokio::spawn(async move {
            let room = self.keeper.room().clone();
            if !self.keeper.memory.merges.start(&room) {
                return;
            }
            let settings = MemorySettings::from(&self.config.resolved.values());
            let sessions = usize::try_from(settings.merge_sessions).unwrap_or(usize::MAX);
            // 出错的当场记了 `WARN`：真相没动，下次抽完再来。
            if let Err(why) = self.run(Some((settings.merge_every, sessions))).await {
                tracing::debug!(target: TARGET, room = %room, error = why.as_str(), "memory merge stopped");
            }
            self.keeper.memory.merges.finish(&room);
        });
    }

    /// 现在就整理（施工 R-7 补）：不看会话数、间隔，合到没有剩下的，交回几样数。
    ///
    /// # Errors
    ///
    /// 人格记忆没装（[`NotDreamed::Off`]）；这一间正在合（[`NotDreamed::Busy`]）；有一次没成（[`NotDreamed::Failed`]，之前
    /// 合成的几次照样记下了）。
    pub(crate) async fn now(self) -> Result<Dreamed, NotDreamed> {
        if !self.keeper.installed() {
            return Err(NotDreamed::Off);
        }
        let room = self.keeper.room().clone();
        if !self.keeper.memory.merges.start(&room) {
            return Err(NotDreamed::Busy);
        }
        let done = self.run(None).await.map_err(NotDreamed::Failed);
        self.keeper.memory.merges.finish(&room);
        done
    }

    /// 合到没有剩下的，交回合了多少：`check` 是头一批看的间隔和会话数（没有的不看），接着合的只看还有没有新记的（放不下的、
    /// 合着的时候又记的）。
    ///
    /// # Errors
    ///
    /// 记忆日志读不了、有一次没成：英文的一句为什么，当场记了 `WARN`。
    async fn run(&self, mut check: Option<(Duration, usize)>) -> Result<Dreamed, String> {
        let (manual, started, mut batches) = (check.is_none(), Instant::now(), 0_u32);
        // 轮到的旧记忆（施工 R-19）：一趟开头照底账挑一次，头一个放得下的批带上；新记的塞满了的照这一份往后挪，不重挑（重挑会把
        // 这一趟刚合完的算成旧的）。
        let mut sweep = Sweep::Pick;
        let room = self.keeper.room().to_string();
        let mut done = Dreamed::default();
        loop {
            if self.hush.heard() {
                done.stopped = true;
                tracing::info!(target: TARGET, room = room.as_str(), batches, "memory merging stopped");
                return Ok(done);
            }
            let keeper = self.keeper.clone();
            let picked = std::mem::replace(&mut sweep, Sweep::Done);
            let found = blocking(move || keeper.merge_batch(wall_now(), check, picked)).await;
            let batch = match found {
                Ok(Some(batch)) => batch,
                Ok(None) => {
                    if batches > 0 {
                        let took_ms =
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                        let Dreamed {
                            given,
                            revised,
                            retired,
                            ..
                        } = done;
                        tracing::info!(target: TARGET, room = room.as_str(), batches, given, revised, retired, took_ms, "memory merging ended");
                    }
                    return Ok(done);
                }
                Err(refused) => {
                    tracing::warn!(target: TARGET, room = %self.keeper.room(), error = ?refused, "memory merge failed");
                    return Err(format!("{refused:?}"));
                }
            };
            check = None;
            let left = u32::try_from(batch.fresh.len()).unwrap_or(u32::MAX);
            if batches == 0 {
                tracing::info!(target: TARGET, room = room.as_str(), left, manual, "memory merging started");
            }
            batches += 1;
            let carried = batch.sweep.clone();
            let asked = self.compose(batch).await;
            if asked.swept.is_none() && !carried.is_empty() {
                sweep = Sweep::Carry(carried);
            }
            // 发以前先记一行开始合（施工 R-17）：订阅着的头照它画进度；带上这一批交的是哪几条（施工 R-21）。
            let (keeper, upto, fresh) = (self.keeper.clone(), asked.upto, asked.fresh);
            let given = asked.given.iter().copied().collect();
            blocking(move || keeper.record_merging(wall_now(), upto, fresh, left, manual, given))
                .await
                .map_err(|refused| format!("{refused:?}"))?;
            let round = self.ask(&asked).await?;
            done.given += round.given;
            done.revised += round.revised;
            done.retired += round.retired;
            done.summary |= round.summary;
            done.stopped |= round.stopped;
        }
    }

    /// 发、读、遮、记；交回这一次合了多少。
    ///
    /// # Errors
    ///
    /// 发不出去、交回读不成、记忆日志写不进：英文的一句为什么，当场记了 `WARN`。
    async fn ask(&self, asked: &Asked) -> Result<Dreamed, String> {
        let started = Instant::now();
        let room = self.keeper.room().to_string();
        let secrets = secrets(&self.config);
        let shapes = &self.extraction.shapes;
        let organizer = MemorySettings::from(&self.config.resolved.values()).organizer;
        tracing::info!(target: TARGET, room = room.as_str(), given = asked.given.len(), fresh = asked.fresh, "memory merge started");
        let ask = Ask {
            model: organizer,
            purpose: PURPOSE.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                blocks: vec![Block::Text(Text {
                    text: redact(&asked.text, &secrets, shapes),
                })],
            }],
            max_tokens: None,
            owner: self.owner.clone(),
        };
        // 边收边读，写完一条推一条（施工 R-21）：遮过 key，不落盘。
        let mut watch = Watch::new(asked.given.clone(), TEXT_CHARS);
        let mut tell = |watched: Watched<'_>| {
            let seen = match watched {
                Watched::Text(text) => watch.feed(text),
                Watched::Restart => {
                    watch.restart();
                    return;
                }
            };
            for seen in seen {
                let seen = match seen {
                    Seen::Revised(mut revised) => {
                        revised.text = redact(&revised.text, &secrets, shapes);
                        Seen::Revised(revised)
                    }
                    Seen::Retired(mut retire) => {
                        retire.why = redact(&retire.why, &secrets, shapes);
                        Seen::Retired(retire)
                    }
                    summary @ Seen::Summary { .. } => summary,
                };
                let dreaming = Dreaming {
                    upto: asked.upto,
                    seen,
                };
                self.keeper.memory.dreams.tell(self.keeper.room(), dreaming);
            }
        };
        let calling =
            self.extraction
                .ask
                .call_watching(&self.config, &self.extraction.blobs, ask, &mut tell);
        // 人叫停了：这一批放弃（施工 R-22），请求跟着丢掉，同打断一轮。
        let answered = tokio::select! {
            answered = calling => answered,
            () = self.hush.wait() => return self.hushed(asked.upto).await,
        };
        let answered = answered
            .map_err(|unanswered| unanswered.reason().to_string())
            .and_then(|answer| decisions(&answer.text, &asked.given, TEXT_CHARS, SUMMARY_CHARS));
        let merges = &self.keeper.memory.merges;
        let given = u32::try_from(asked.given.len()).unwrap_or(u32::MAX);
        match answered {
            Ok(mut decided) => {
                for revised in &mut decided.revised {
                    revised.text = redact(&revised.text, &secrets, shapes);
                }
                for retire in &mut decided.retired {
                    retire.why = redact(&retire.why, &secrets, shapes);
                }
                if let Some(summary) = &mut decided.summary {
                    *summary = redact(summary, &secrets, shapes);
                }
                let summary = decided.summary.is_some();
                let (keeper, upto, swept) = (self.keeper.clone(), asked.upto, asked.swept.clone());
                let recorded = blocking(move || {
                    keeper.record_merge(wall_now(), &decided, upto, given, swept.as_ref())
                })
                .await;
                merges.record(self.keeper.room(), true);
                match recorded {
                    Ok((revised, retired)) => {
                        let took_ms =
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                        tracing::info!(target: TARGET, room = room.as_str(), revised, retired, took_ms, "memory merged");
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
                            stopped: false,
                        })
                    }
                    Err(refused) => {
                        tracing::warn!(target: TARGET, room = room.as_str(), error = ?refused, "memory merge not recorded");
                        Err(format!("{refused:?}"))
                    }
                }
            }
            Err(why) => {
                let tries = merges.record(self.keeper.room(), false);
                tracing::warn!(target: TARGET, room = room.as_str(), tries, error = why.as_str(), "memory merge failed");
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
                    tracing::warn!(target: TARGET, room = room.as_str(), error = ?refused, "memory merge not recorded");
                }
                Err(why)
            }
        }
    }

    /// 人叫停了这一批（施工 R-22）：记一条 `stopped`，交回什么都没合。写不进的记一行，照样算停了。
    async fn hushed(&self, upto: Seq) -> Result<Dreamed, String> {
        let keeper = self.keeper.clone();
        if let Err(refused) = blocking(move || keeper.record_merge_stopped(wall_now(), upto)).await
        {
            tracing::warn!(target: TARGET, room = %self.keeper.room(), error = ?refused, "memory merge not recorded");
        }
        Ok(Dreamed {
            stopped: true,
            ..Dreamed::default()
        })
    }
}

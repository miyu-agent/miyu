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

use miyu_models::side::{self, ORGANIZE};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use miyu_kernel::id::{AccountId, Seq, SessionId};
use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{Entry, MemoryId, Merging, Swept};
use miyu_store::recall::Room;
use miyu_tool::load::{self, LoadError};

use super::{DreamResult, DreamRoom, DreamSpec, Extraction, Keeper};
use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::config::TurnConfig;
use crate::settings::MemorySettings;

mod ask;
mod compose;
mod hush;
mod land;
mod merges;
mod people;

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
    /// 这一批里有认识的人时接在最后的那一段（施工 E-1 下）：有谁、怎么合他们的卡、交回的样子。
    people: Template,
    /// 他们现在的卡那一块的标题，和一张一行（施工 E-1 下）。
    cards: Template,
    card: Template,
    line: Template,
    unnamed: Template,
}

impl MergeTexts {
    /// 从资源目录 `resources` 读：`merge/instruction.txt`（`{owner}`）、`summary.txt`（`{text}`）、`none.txt`、`new.txt`、
    /// `related.txt`、`sweep.txt`（施工 R-19）、`people.txt`（`{people}`）、`cards.txt`、`card.txt`（`{who}`、`{text}`，施工 E-1 下），一行照 `memory_search/memory.txt`（`{id}`、`{class}`、`{date}`、`{text}`），没有称呼时主人写的那一句
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
            people: text("people", &["people"])?,
            cards: text("cards", &[])?,
            card: text("card", &["who", "text"])?,
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// 这一趟跑在哪个整理会话里（施工 D-2，设计 41）：没有要整理的、走一次性入口的没有。
    pub session: Option<SessionId>,
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
            from: None,
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
    /// 人在哪个会话里叫的（设计 41 DS1 的 `dream.from`）：抽完起的是那个会话；不经会话的没有。只有人叫的那一趟记它。
    pub(crate) from: Option<SessionId>,
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

/// 交出去的一次：请求的字、交进去的编号、合到第几条、其中新记的几条、顺带重看到哪（施工 R-19）、这一批里的认识的人（施工
/// E-1 下：照是谁，交回的卡只收他们的；认出来的名字记进卡的叫法）。
struct Asked {
    text: String,
    given: BTreeSet<MemoryId>,
    upto: Seq,
    fresh: u32,
    swept: Option<Swept>,
    people: BTreeSet<String>,
    known: people::People,
}

impl MergeJob {
    /// 抽取记下以后调：够了的在后台合（`memory.md` 第七条第 1 款）。同一间已经在合的、不够的什么都不做。
    pub(crate) fn start(self) {
        tokio::spawn(async move {
            let room = self.keeper.room().clone();
            let Some(_running) = Merges::begin(&self.keeper.memory, &room) else {
                return;
            };
            let settings = MemorySettings::from(&self.config.resolved.values());
            let sessions = usize::try_from(settings.merge_sessions).unwrap_or(usize::MAX);
            // 出错的当场记了 `WARN`：真相没动，下次抽完再来。
            if let Err(why) = self.run(Some((settings.merge_every, sessions))).await {
                tracing::debug!(target: TARGET, room_kind = room.kind(), room_id = self.keeper.room_id().as_str(), error = why.as_str(), "memory merge stopped");
            }
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
        let running = Merges::begin(&self.keeper.memory, &room).ok_or(NotDreamed::Busy)?;
        // 在自己的任务里合（施工 R-30）：叫的那一头半路不等了（连接断了）照样合完、放锁，不然这一间一直忙。
        let task = tokio::spawn(async move {
            let _running = running;
            self.run(None).await.map_err(NotDreamed::Failed)
        });
        task.await
            .unwrap_or_else(|_| Err(NotDreamed::Failed("memory merge task failed".to_string())))
    }

    /// 合到没有剩下的，交回合了多少：`check` 是头一批看的间隔和会话数（没有的不看），接着合的只看还有没有新记的（放不下的、
    /// 合着的时候又记的）。
    ///
    /// # Errors
    ///
    /// 记忆日志读不了、有一次没成：英文的一句为什么，当场记了 `WARN`。
    async fn run(&self, check: Option<(Duration, usize)>) -> Result<Dreamed, String> {
        let mut trip = None;
        let done = self.batches(check, &mut trip).await;
        if let Some(trip) = trip {
            let result = done.as_ref().ok().map(|done| DreamResult {
                given: done.given,
                revised: done.revised,
                retired: done.retired,
                summary: done.summary,
                stopped: done.stopped,
            });
            self.close_trip(trip, result).await;
        }
        done
    }

    /// 一批一批合到没有剩下的（[`MergeJob::run`]）：开了的整理会话放在 `trip` 里，跑完由 `run` 收。
    ///
    /// # Errors
    ///
    /// 同 [`MergeJob::run`]。
    async fn batches(
        &self,
        mut check: Option<(Duration, usize)>,
        trip: &mut Option<ask::Trip>,
    ) -> Result<Dreamed, String> {
        let (manual, started, mut batches) = (check.is_none(), Instant::now(), 0_u32);
        // 轮到的旧记忆（施工 R-19）：一趟开头照底账挑一次，头一个放得下的批带上；新记的塞满了的照这一份往后挪，不重挑（重挑会把
        // 这一趟刚合完的算成旧的）。
        let mut sweep = Sweep::Pick;
        let (room_kind, room_id) = (self.keeper.room().kind(), self.keeper.room_id());
        let mut done = Dreamed::default();
        // 这一趟的整理会话（设计 41）：真有要合的批时才开，一趟一个；没接会话表的走一次性入口。
        loop {
            if self.hush.heard() {
                done.stopped = true;
                tracing::info!(target: TARGET, room_kind, room_id = room_id.as_str(), batches, "memory merging stopped");
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
                        } = &done;
                        tracing::info!(target: TARGET, room_kind, room_id = room_id.as_str(), batches, given, revised, retired, took_ms, "memory merging ended");
                    }
                    return Ok(done);
                }
                Err(refused) => {
                    tracing::warn!(target: TARGET, room_kind = self.keeper.room().kind(), room_id = self.keeper.room_id().as_str(), error = ?refused, "memory merge failed");
                    return Err(format!("{refused:?}"));
                }
            };
            check = None;
            let left = u32::try_from(batch.fresh.len()).unwrap_or(u32::MAX);
            if batches == 0 {
                tracing::info!(target: TARGET, room_kind, room_id = room_id.as_str(), left, manual, "memory merging started");
            }
            batches += 1;
            let carried = batch.sweep.clone();
            let asked = self.compose(batch).await;
            if asked.swept.is_none() && !carried.is_empty() {
                sweep = Sweep::Carry(carried);
            }
            if trip.is_none()
                && let Some(opened) = self.open_trip(manual).await?
            {
                done.session = Some(opened.session.clone());
                *trip = Some(opened);
            }
            // 发以前先记一行开始合（施工 R-17）：订阅着的头照它画进度；带上这一批交的是哪几条（施工 R-21）、跑在哪个整理会话里
            // （施工 D-2）。
            let keeper = self.keeper.clone();
            let merging = Merging {
                upto: asked.upto,
                fresh: asked.fresh,
                left,
                manual,
                given: asked.given.iter().copied().collect(),
                session: trip.as_ref().map(|trip| trip.session.clone()),
            };
            blocking(move || keeper.record_merging(wall_now(), merging))
                .await
                .map_err(|refused| format!("{refused:?}"))?;
            let round = self.ask(&asked, trip.as_ref(), batches).await?;
            done.given += round.given;
            done.revised += round.revised;
            done.retired += round.retired;
            done.summary |= round.summary;
            if round.stopped {
                // 放弃了这一批（人叫停、整理会话自己停了）：这一趟到此为止，下次从它接着合。
                done.stopped = true;
                tracing::info!(target: TARGET, room_kind, room_id = room_id.as_str(), batches, "memory merging stopped");
                return Ok(done);
            }
        }
    }

    /// 开这一趟的整理会话（设计 41 第二节）：接了会话表的才开；人叫的记是哪个会话叫的。
    ///
    /// # Errors
    ///
    /// 开不了（核心在停、写不进）：这一趟停，下次再合。
    async fn open_trip(&self, manual: bool) -> Result<Option<ask::Trip>, String> {
        let Some(sessions) = self.keeper.memory.dream_sessions() else {
            return Ok(None);
        };
        let (owner, room) = match self.keeper.room() {
            Room::Persona { account, persona } => {
                (account.clone(), DreamRoom::Persona(persona.clone()))
            }
            Room::Session { account, session } => {
                (account.clone(), DreamRoom::Session(session.clone()))
            }
        };
        let spec = DreamSpec {
            owner,
            room,
            manual,
            from: if manual { self.from.clone() } else { None },
            // 整理会话钉登记的档位池（施工 Q-2）：池是空的照 `models.chat`；全在冷却照主请求报。
            model: side::resolved(&self.config.resolved.values(), &ORGANIZE)
                .map(|_| side::reference(&ORGANIZE)),
        };
        match sessions.open(spec).await {
            Ok(session) => Ok(Some(ask::Trip {
                sessions: Arc::clone(sessions),
                session,
            })),
            Err(why) => {
                tracing::warn!(target: TARGET, room_kind = self.keeper.room().kind(), room_id = self.keeper.room_id().as_str(), error = why.as_str(), "dream session not opened");
                Err(why)
            }
        }
    }

    /// 这一趟跑完了（设计 41 DS9，施工 D-1 下的 `close`）：照 `memory.dream_keep` 只留这一间最近几趟，交上这一趟做了什么
    /// （`result`，施工 DR-1、D-2 补：合完的、叫停的是回执的几样数，出错没跑完的是空的，核心照它记不记 `dream.ended`）。收不成
    /// 的记一行，不影响回执。
    async fn close_trip(&self, trip: ask::Trip, result: Option<DreamResult>) {
        let keep = MemorySettings::from(&self.config.resolved.values()).dream_keep;
        let keep = usize::try_from(keep).unwrap_or(0);
        if let Err(why) = trip.sessions.close(trip.session, keep, result).await {
            tracing::warn!(target: TARGET, room_kind = self.keeper.room().kind(), room_id = self.keeper.room_id().as_str(), error = why.as_str(), "dream session not closed");
        }
    }
}

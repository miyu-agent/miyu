//! 一间记忆和这次谁在听（施工 R-3 补，`docs/blueprint/memory.md`「工具」「协议」、第二条、第九条）：记、改、忘、清空、搜、列
//! 都在这里。她的三件工具（`port.rs`）和协议（端点的 `memory.*`、`/remember`）都调它，只是 `by`、出处、听众不一样。碰记忆日志
//! 和检索库，调的一方放在阻塞线程里。
//!
//! - 谁在听（施工 R-13 上，[`Listener`]）只管记下时写什么；哪儿都想得起全部（施工 R-29，2026-10-21 项目主人：记忆是一个整体）。
//! - 出处活不活照回合库的墓碑（第二条第 4 款）：一处都没有（人记的）的算活；有的，活着一处就算。

use std::sync::{Arc, Mutex};

use miyu_kernel::event::Event;
use miyu_kernel::id::{CommandId, ModuleId, Seq, SessionId, TurnId};
use miyu_kernel::origin::{By, Module, Person};
use miyu_kernel::time::Timestamp;
use miyu_recall::extract::Candidate;
use miyu_recall::{
    Cleared, EPISODE, Entry, Extracted, MemoryEvent, MemoryId, PERSON, Retired, Saved, Skipped,
    Source,
};
use miyu_store::memory::{MemoryLog, Report};
use miyu_store::recall::{Opened, Room};
use miyu_tool::{Refused, Remember};

use crate::TARGET;

use super::Memory;
use super::listener::Listener;
use super::vectors::{Query, Target, Using, Vectors};

mod search;
mod tag;
mod update;

/// 一间记忆，连同这次会被谁看到（听众）。可以复制：每次调用照它做。
#[derive(Debug, Clone)]
pub struct Keeper {
    pub(super) memory: Arc<Memory>,
    room: Room,
    listener: Listener,
    /// 这个会话里她最近在场所里叫什么（施工 E-1 修三，`her.rs`）：会话的记忆端口照它记下的人话更新，复制出来的共用一份；协议
    /// 造的、没有会话的是空的。
    here: Arc<Mutex<Option<String>>>,
}

/// 谁、什么时候、经哪条命令写的。
#[derive(Debug, Clone)]
pub struct Stamp {
    /// 时刻。
    pub at: Timestamp,
    /// 写它的那一方：她经工具记的是那次调用，人记的是那个人。
    pub by: By,
    /// 人经协议、斜杠命令写的，那条命令的编号：同一个编号再来只算一次，交回头一次的结果（04 第六节第 1 条）。她经工具写的
    /// 没有。
    pub cause: Option<CommandId>,
}

/// 列出的条件（协议的 `memory.list`）。
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// 只要这一类。
    pub class: Option<String>,
    /// 只要出处在这个会话里的。
    pub from: Option<SessionId>,
    /// 作废的也要。
    pub forgotten: bool,
    /// 最多几条。
    pub limit: usize,
}

impl Keeper {
    /// 记忆 `memory` 里的 `room` 那一间，这次是 `listener` 在听。
    pub fn new(memory: &Arc<Memory>, room: Room, listener: Listener) -> Keeper {
        Keeper {
            memory: Arc::clone(memory),
            room,
            listener,
            here: Arc::default(),
        }
    }

    /// 哪一间。
    pub fn room(&self) -> &Room {
        &self.room
    }

    /// 这一间里她叫什么（施工 E-1 修，`her.rs`）：照 `fold` 过的，没交过的是空的。这个会话里她最近在场所里叫的那个也算
    /// （施工 E-1 修三）。
    pub(crate) fn her(&self) -> std::collections::BTreeSet<String> {
        let mut names = self.memory.her(&self.room);
        let here = self
            .here
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(name) = here.map(|name| super::her::fold(&name))
            && !name.is_empty()
        {
            names.insert(name);
        }
        names
    }

    /// 记下的事件 `events` 里有她在场所里叫什么的（施工 E-1 修三），记住最后那一个；没有的不动。
    pub(crate) fn hear(&self, events: &[Event]) {
        self.called(super::her::last_called(events));
    }

    /// 她在这个会话里最近叫 `name`（施工 E-1 修三）：没有的不动。
    pub(crate) fn called(&self, name: Option<String>) {
        if let Some(name) = name {
            *self
                .here
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(name);
        }
    }

    /// 运行日志里这一间的短号（施工 LG-1 下）：和 [`Room::kind`] 一起写，不写账号、人格名、会话编号。
    pub(crate) fn room_id(&self) -> String {
        self.memory.room_id(&self.room)
    }

    /// 记一条：出处 `sources`，听众是这次的听众。写了 `replaces` 的是改那一条：它得现在算数、听众合。交回新的编号；这条命令
    /// 做过的交回那一次的（记忆日志认编号）。认识的人那一类、说的是她自己的（`who` 是这一间她的名字，施工 E-1 修）改记成
    /// `episode`、不写是谁。
    ///
    /// # Errors
    ///
    /// `replaces` 没有这一条（听众不合的也当没有）、已经不算了；记忆日志开不了、写不进。
    pub fn save(
        &self,
        stamp: Stamp,
        remember: Remember,
        sources: Vec<Source>,
    ) -> Result<MemoryId, Refused> {
        let log = self.log()?;
        // 改一条没给关键词、是谁的，带上旧的（施工 R-15 上、E-1 上）：说的还是那件事、那个人；发生过的事涉及谁照旧带上（施工
        // E-1 修四：她的工具不收涉及谁）。
        let (carried, carried_who, carried_people) = match remember.replaces {
            Some(old) => {
                let old = self.current(&log, old)?;
                (old.keywords, old.who, old.people)
            }
            None => (None, None, Vec::new()),
        };
        let who = (remember.class == PERSON)
            .then(|| remember.who.filter(|who| !who.is_empty()).or(carried_who))
            .flatten();
        // 记的认识的人是她自己的（施工 E-1 修）：改记成关于她自己的事，不写是谁。
        let (class, who) = match &who {
            Some(name) if super::her::is_her(&self.her(), name) => {
                (super::her::SELF_CLASS.to_string(), None)
            }
            _ => (remember.class, who),
        };
        let people = match class == EPISODE {
            true => carried_people,
            false => Vec::new(),
        };
        let (audience, venue) = self.listener.heard();
        let saved = Saved {
            class,
            text: remember.text,
            sources,
            audience,
            replaces: remember.replaces,
            about: None,
            venue,
            keywords: match remember.keywords {
                Some(keywords) if keywords.is_empty() => None,
                Some(keywords) => Some(keywords),
                None => carried,
            },
            who,
            people,
        };
        self.append(&log, stamp, &MemoryEvent::Saved(saved))
            .map(|(id, _)| id)
    }

    /// 抽取要的几样（施工 R-6 上）：核心没交的没有，不抽。
    pub(crate) fn extraction(&self) -> Option<super::Extraction> {
        self.memory.extraction().cloned()
    }

    /// 人格记忆这时装着没有（施工 R-10）：没装的开着的会话不交摘要、不抽。
    pub(crate) fn installed(&self) -> bool {
        self.memory.installed()
    }

    /// 会话 `session` 抽到了它日志的第几条（施工 R-6 上，`memory.md` 第六条）；还没抽过的是 `None`。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了。
    pub(crate) fn extracted(&self, session: &SessionId) -> Result<Option<Seq>, Refused> {
        Ok(self.log()?.book(|book| book.extracted(session)))
    }

    /// 记下会话 `session` 抽出来的几条（施工 R-6 上）：`by` 是记忆模块，出处各是那一轮，听众是这次的听众；再记一条它抽到了
    /// 第 `upto` 条（整段跳过的带上为什么）。交回记下的那几条的编号（施工 R-15 下：接着给它们写关键词）。候选在前、记号在后：
    /// 中途写不进的，下次重抽这一段。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进。
    pub(crate) fn record_extraction(
        &self,
        at: Timestamp,
        session: &SessionId,
        upto: Seq,
        found: Vec<Candidate>,
        skipped: Option<Skipped>,
    ) -> Result<Vec<MemoryId>, Refused> {
        let log = self.log()?;
        let by = By::Module(Module {
            id: ModuleId::parse(super::MODULE).expect("合模块编号的写法"),
        });
        let mut saved_ids = Vec::new();
        for candidate in found {
            let Some(turn) = Seq::new(candidate.turn).map(TurnId::new) else {
                continue;
            };
            let (audience, venue) = self.listener.heard();
            let saved = Saved {
                class: candidate.class,
                text: candidate.text,
                sources: vec![Source {
                    session: session.clone(),
                    turn,
                }],
                audience,
                replaces: None,
                about: candidate.about,
                venue,
                keywords: None,
                who: candidate.who,
                people: candidate.people,
            };
            let stamp = Stamp {
                at,
                by: by.clone(),
                cause: None,
            };
            let (id, _) = self.append(&log, stamp, &MemoryEvent::Saved(saved))?;
            saved_ids.push(id);
        }
        let mark = Extracted {
            session: session.clone(),
            upto,
            count: u32::try_from(saved_ids.len()).unwrap_or(u32::MAX),
            skipped,
        };
        let stamp = Stamp {
            at,
            by,
            cause: None,
        };
        self.append(&log, stamp, &MemoryEvent::Extracted(mark))?;
        Ok(saved_ids)
    }

    /// 作废一条。
    ///
    /// # Errors
    ///
    /// 没有这一条（听众不合的也当没有）、已经不算了；记忆日志开不了、写不进。
    pub fn retire(&self, stamp: Stamp, id: MemoryId, why: String) -> Result<(), Refused> {
        let log = self.log()?;
        if done(&log, &stamp).is_some() {
            return Ok(());
        }
        self.current(&log, id)?;
        self.append(
            &log,
            stamp,
            &MemoryEvent::Retired(Retired {
                id,
                why,
                kind: None,
                kept: None,
            }),
        )
        .map(|_| ())
    }

    /// 清空（第二条第 5 款）：`session` 是只清从这个会话来的，没有的是整间。交回清掉了几条。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进。
    pub fn clear(&self, stamp: Stamp, session: Option<SessionId>) -> Result<usize, Refused> {
        let log = self.log()?;
        self.append(&log, stamp, &MemoryEvent::Cleared(Cleared { session }))
            .map(|(_, cleared)| cleared)
    }

    /// 照条件列出，新的在前：给的规矩同 [`Keeper::search`]。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了。
    pub fn list(&self, filter: &Filter) -> Result<Vec<Entry>, String> {
        let log = self.log().map_err(failed)?;
        let all = log.book(|book| book.all().rev().cloned().collect::<Vec<Entry>>());
        Ok(all
            .into_iter()
            .filter(|entry| {
                filter
                    .class
                    .as_ref()
                    .is_none_or(|class| &entry.class == class)
            })
            .filter(|entry| {
                filter
                    .from
                    .as_ref()
                    .is_none_or(|from| entry.sources.iter().any(|source| &source.session == from))
            })
            .filter(|entry| self.shown(entry, filter.forgotten))
            .take(filter.limit)
            .collect())
    }

    /// `since` 以后忘掉的（作废的、改掉的旧版本）：听众合、没清掉的，新的在前最多 `limit` 条（施工 R-6 修，
    /// `memory.md` 第六条第 4 款）。抽取照它告诉模型这些别再记。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了。
    pub(crate) fn dropped_since(
        &self,
        since: Timestamp,
        limit: usize,
    ) -> Result<Vec<Entry>, Refused> {
        let log = self.log()?;
        let mut dropped: Vec<(Timestamp, Entry)> = log.book(|book| {
            book.all()
                .filter_map(|entry| {
                    let at = book.dropped_at(entry.id).filter(|at| *at >= since)?;
                    Some((at, entry.clone()))
                })
                .collect()
        });
        dropped.retain(|(_, entry)| !entry.cleared);
        // 同一个时刻忘掉的照编号：后记的在前。
        dropped.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.id.cmp(&a.1.id)));
        Ok(dropped
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry)
            .collect())
    }

    /// 照意思找的那一路照 `using` 在后台补这一间缺的向量（施工 R-5 下，`vectors.rs`）：搜的时候起。没接向量的、`off` 的什么
    /// 都不做。
    pub fn fill(&self, using: &Using) {
        let Some(vectors) = self.memory.vectors() else {
            return;
        };
        if let Ok(log) = self.log() {
            vectors.fill(
                using,
                format!("memories {:?}", self.room),
                Target::Memories(log),
            );
        }
        let (turns, _) = self.memory.turns.turns(&self.room);
        vectors.fill(
            using,
            format!("turns {:?}", self.room),
            Target::Turns(turns),
        );
    }

    /// 照意思找的那一路（施工 R-5 下）：核心没接的没有。
    pub fn vectors(&self) -> Option<&Arc<Vectors>> {
        self.memory.vectors()
    }

    /// 这一条给不给看：没改掉、没清掉（`forgotten` 时作废的也给）、出处活着。哪儿都给（施工 R-29）。
    pub(super) fn shown(&self, entry: &Entry, forgotten: bool) -> bool {
        entry.replaced_by.is_none()
            && !entry.cleared
            && (forgotten || entry.retired.is_none())
            && self.alive(entry)
    }

    /// 这一间的记忆日志；这一回第一次开的记一行派生的情形。
    pub(super) fn log(&self) -> Result<Arc<MemoryLog>, Refused> {
        let (log, report) = self
            .memory
            .logs
            .open(&self.room)
            .map_err(|error| Refused::Failed(error.to_string()))?;
        if let Some(report) = report {
            log_report(&self.room, &self.room_id(), &report);
            self.close_interrupted(&log, crate::clock::wall_now());
        }
        Ok(log)
    }

    /// 编号 `id` 的那一条现在算数，交回它：没有的当没有；改掉的、作废的、清掉的说已经不算了。
    fn current(&self, log: &MemoryLog, id: MemoryId) -> Result<Entry, Refused> {
        match log.book(|book| book.get(id).cloned()) {
            Some(entry) => {
                if entry.current() {
                    Ok(entry)
                } else {
                    Err(Refused::NotCurrent(id))
                }
            }
            _ => Err(Refused::NoSuch(id)),
        }
    }

    /// 追加一条，交回编号和清掉了几条；记忆库没写上的记一行，不挡。
    pub(super) fn append(
        &self,
        log: &MemoryLog,
        stamp: Stamp,
        event: &MemoryEvent,
    ) -> Result<(MemoryId, usize), Refused> {
        let appended = log
            .append(stamp.at, stamp.by, stamp.cause.as_ref(), event)
            .map_err(|error| Refused::Failed(error.to_string()))?;
        if let Err(error) = appended.indexed {
            tracing::warn!(target: TARGET, error = %error, "memory index not updated");
        }
        Ok((appended.id, appended.cleared))
    }

    /// 同一间、主人那边在听的一份（施工 R-13 上）：合并照全部合，不管是哪个会话抽完以后起的。
    pub(crate) fn as_owner(&self) -> Keeper {
        let account = match &self.room {
            Room::Persona { account, .. } | Room::Session { account, .. } => account,
        };
        Keeper {
            memory: Arc::clone(&self.memory),
            room: self.room.clone(),
            listener: Listener::Owner(By::Person(Person::new(account.clone()))),
            here: Arc::clone(&self.here),
        }
    }

    /// 出处还活着：一处都没有的算活；有的，活着一处就算。回合库读不了的当活的：宁可多想起来，不吞掉人说过的。
    pub(super) fn alive(&self, entry: &Entry) -> bool {
        entry.sources.is_empty()
            || entry
                .sources
                .iter()
                .any(|source| self.memory.turns.alive(&self.room, source).unwrap_or(true))
    }
}

/// 这条命令做过的（`stamp.cause`）：那一条的编号、清掉几条。先看它，再查参数：改一条、作废一条重发时，那一条已经不算了。
pub(super) fn done(log: &MemoryLog, stamp: &Stamp) -> Option<(MemoryId, usize)> {
    let cause = stamp.cause.as_ref()?;
    log.book(|book| book.done(cause))
}

/// 开不了日志说成一句。
pub(super) fn failed(refused: Refused) -> String {
    match refused {
        Refused::Failed(why) => why,
        other => format!("{other:?}"),
    }
}

/// 记忆日志这一回第一次开：派生的那些是什么情形，照会话列表的索引的说法记。哪一间写种类和短号（施工 LG-1 下）。
fn log_report(room: &Room, room_id: &str, report: &Report) {
    let room_kind = room.kind();
    match &report.index {
        Opened::Kept => {}
        Opened::Created => {
            tracing::info!(target: TARGET, room_kind, room_id, "memory log index created")
        }
        Opened::Rebuilt(why) => {
            tracing::warn!(target: TARGET, room_kind, room_id, reason = %why, "memory log index rebuilt")
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, room_kind, room_id, error = %error, "memory log index unusable")
        }
    }
    if let Some(error) = &report.caught_up {
        tracing::warn!(target: TARGET, room_kind, room_id, error = %error, "memory log index not caught up");
    }
    for (seq, why) in &report.unreadable {
        tracing::warn!(target: TARGET, room_kind, room_id, seq = seq.get(), why = why.as_str(), "memory event unreadable");
    }
    if report.filled > 0 {
        tracing::info!(target: TARGET, room_kind, room_id, filled = report.filled, "memory log index caught up");
    }
}

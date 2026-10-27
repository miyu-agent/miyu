//! 什么时候抽、这一段从哪来（施工 R-6 上，`docs/blueprint/memory.md` 第六条第 1 款；抽本身在 `memory/extract.rs`）。
//!
//! 1. 会话从忙到闲那一刻（`watchers.rs` 的 `after_batch`）上一个闹钟，`memory.extract_idle` 以后响；忙起来就撤掉。只给记忆
//!    开着、核心交过抽取要的几样的会话（本机的主会话：别的没有记忆的端口）。响的时候人格记忆没装的不抽（施工 R-10）：没装时上
//!    的、上了以后卸掉的都在这里挡下。
//! 2. 响了、还闲着、没有一次在路上：在阻塞线程里读记忆日志里它抽到了哪、会话日志里那以后的事件（不占 actor，不碰 actor 手里
//!    的日志）。
//! 3. 读回来、还是那个闹钟、还闲着：拼一份留着一切的历史（`History::whole`，压缩替代掉的也在），交给会话的组装器渲染这一段，
//!    定怎么办，派出去。读的时候来了新一轮的这一次作废，等下次闲了再来。拼、渲染、定在阻塞线程里做（施工 PERF-4，`planned.rs`），
//!    不占会话。
//! 4. 一直不闲的会话（施工 E-5）：每送完一批看一眼，上次开始抽以后结束了 `memory.extract_max_turns` 轮、或者最早那一轮结束
//!    满 `memory.extract_max_wait`，不等闲就上一次不等闲的：不看闲不闲、新一轮开始不撤它，读回来只取到读的那一刻结束了的
//!    最后一轮（正在跑的那一轮不交，留给下次）。抽取本来就不进内核、不挡会话。

use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;
use tracing::Instrument;

use tokio::sync::oneshot;

use super::back::answer_back;
use super::{Actor, answer};
use crate::config::Turn;
use crate::memory::{Dreamed, Extractor, Hush, Job, NotDreamed, called, ended_only};
use crate::port::Back;
use crate::settings::MemorySettings;
use planned::{Planning, planned};

impl Actor {
    /// 群会话的群聊近况预算（施工 R-13 上，`memory.md` 第六条第 4 款）：抽取照它每一轮只带她当时看到的那几句；不是群会话的
    /// 没有。快照里群聊近况没有字的（O-13 中造的群会话）不限。
    fn group_budget(&self) -> Option<usize> {
        let group = self.persona.as_ref()?.snapshot.group.as_ref()?;
        Some(group.recent.as_ref().map_or(usize::MAX, |recent| {
            usize::try_from(recent.budget).unwrap_or(usize::MAX)
        }))
    }

    /// 群里每一轮带多少（字节预算、最多几条，施工 O-13 再补）：定这一段里人这边每一句是谁说的（[`planned`]）。
    fn voicing(&self) -> (Option<usize>, Option<usize>) {
        let limit = self
            .persona
            .as_ref()
            .and_then(|refresh| refresh.snapshot.group.as_ref()?.recent.as_ref()?.limit)
            .map(|limit| usize::try_from(limit).unwrap_or(usize::MAX));
        (self.group_budget(), limit)
    }

    /// 抽取照称呼写主人（施工 R-6 再修）：这个会话的快照带了「关于你」才用配置 `config` 里的称呼。不带的（群聊、别人的
    /// 会话、桥没写 `profile` 的场所会话）不能把管理员的称呼当成这个人：配置里只有管理员的个人设置。
    fn owner_name(&self, config: &Turn) -> Option<String> {
        self.persona.as_ref()?.snapshot.profile.as_ref()?;
        called(config)
    }

    /// 刚从忙到闲：抽取开着的上一个闹钟。
    pub(super) fn extract_when_idle(&self) {
        let Some((calls, _)) = self.tools.memory() else {
            return;
        };
        let (keeper, _, _, extractor) = calls.extracting();
        if keeper.extraction().is_none() {
            return;
        }
        let idle = keeper
            .extraction()
            .and_then(|extraction| extraction.idle)
            .unwrap_or_else(|| {
                MemorySettings::from(&self.config.current().resolved.values()).extract_idle
            });
        let generation = extractor.arm();
        let idle_ms = u64::try_from(idle.as_millis()).unwrap_or(u64::MAX);
        tracing::debug!(target: crate::TARGET, idle_ms, "memory extraction alarm set");
        let backs = self.backs.clone();
        tokio::spawn(async move {
            tokio::time::sleep(idle).await;
            answer_back(
                &backs,
                Back::ExtractDue {
                    generation,
                    forced: false,
                },
            );
        });
    }

    /// 送完一批（施工 E-5）：一直不闲的会话攒够了轮（`memory.extract_max_turns`）、最早那一轮等够了（`extract_max_wait`），
    /// 不等闲就抽。
    pub(super) fn extract_when_overdue(&mut self) {
        let Some((calls, _)) = self.tools.memory() else {
            return;
        };
        let (keeper, _, _, extractor) = calls.extracting();
        let Some(extraction) = keeper.extraction() else {
            return;
        };
        let settings = MemorySettings::from(&self.config.current().resolved.values());
        let max_turns = u32::try_from(settings.extract_max_turns).unwrap_or(u32::MAX);
        let max_wait = extraction.max_wait.unwrap_or(settings.extract_max_wait);
        let now = self.clock.now();
        if !extractor.overdue(now, max_turns, max_wait) || !extractor.force() {
            return;
        }
        tracing::debug!(target: crate::TARGET, "memory extraction forced while busy");
        answer_back(
            &self.backs,
            Back::ExtractDue {
                generation: 0,
                forced: true,
            },
        );
    }

    /// 记忆的闹钟上着、或者有一次抽取在路上（施工 V-2 再补，`life.rs`）：这时不退下。
    pub(super) fn extracting(&self) -> bool {
        self.tools
            .memory()
            .is_some_and(|(calls, _)| calls.extracting().3.pending())
    }

    /// 叫停这个会话叫的整理（施工 R-22，`/dream stop`、`/stop`）：交回有没有在跑的。
    pub(super) fn hush_dreams(&self) -> bool {
        self.tools
            .memory()
            .is_some_and(|(calls, _)| calls.extracting().3.hushes.hush())
    }

    /// 忙起来了：撤掉闹钟。在路上的那一次照样走完。
    pub(super) fn extract_cancel(&self) {
        if let Some((calls, _)) = self.tools.memory() {
            calls.extracting().3.cancel();
        }
    }

    /// 第 `generation` 个闹钟响了：还算数、还闲着的，在阻塞线程里读这一段。不等闲的那一次（`forced`，施工 E-5）不看第几个、
    /// 闲不闲，只看没有一次在路上。
    pub(super) fn extract_due(&self, generation: u64, forced: bool) {
        let Some((calls, _)) = self.tools.memory() else {
            return;
        };
        let (keeper, session, _, extractor) = calls.extracting();
        let put_down = |extractor: &Extractor| match forced {
            true => extractor.unforce(),
            false => extractor.disarm(generation),
        };
        let dir = self.store.as_ref().and_then(|store| store.dir());
        let Some(dir) = dir else {
            return put_down(extractor);
        };
        let due = match forced {
            true => !extractor.running(),
            false => extractor.due(generation) && self.session.vacant(),
        };
        if !due {
            return put_down(extractor);
        }
        if !keeper.installed() {
            tracing::debug!(target: crate::TARGET, why = "uninstalled", "memory extraction skipped");
            return put_down(extractor);
        }
        let floor = calls.floor();
        let (keeper, session, backs) = (keeper.clone(), session.clone(), self.backs.clone());
        tokio::task::spawn_blocking(move || {
            let read = keeper
                .extracted(&session)
                .map_err(|refused| format!("{refused:?}"))
                .and_then(|after| {
                    let events =
                        miyu_store::log::read_events(&dir).map_err(|error| error.to_string())?;
                    Ok((after.unwrap_or(Seq::FIRST).max(floor), events))
                });
            answer_back(
                &backs,
                Back::ExtractRead {
                    generation,
                    forced,
                    read,
                },
            );
        });
    }

    /// 读回来了：还是那个闹钟、还闲着的，渲染这一段、定怎么办、派出去。不等闲的那一次（`forced`，施工 E-5）不看闲不闲，只
    /// 取到读的那一刻结束了的最后一轮：正在跑的那一轮留给下次。
    pub(super) fn extract_read(
        &self,
        generation: u64,
        forced: bool,
        read: Result<(Seq, Vec<Event>), String>,
    ) {
        let Some((calls, offset)) = self.tools.memory() else {
            return;
        };
        let (keeper, session, owner, extractor) = calls.extracting();
        let Some(extraction) = keeper.extraction() else {
            return match forced {
                true => extractor.unforce(),
                false => extractor.disarm(generation),
            };
        };
        let started = match forced {
            true => extractor.start_forced(),
            false => self.session.vacant() && extractor.start(generation),
        };
        if !started {
            if !forced {
                extractor.disarm(generation);
            }
            return;
        }
        let (after, mut events) = match read {
            Ok(read) => read,
            Err(error) => {
                tracing::warn!(target: crate::TARGET, error = error.as_str(), "memory extraction failed");
                extractor.abandon();
                return;
            }
        };
        if forced {
            ended_only(&mut events);
        }
        let config = Arc::clone(self.config.current());
        let settings = MemorySettings::from(&config.resolved.values());
        let min_turns = usize::try_from(settings.extract_turns).unwrap_or(1);
        let name = self.owner_name(&config);
        let (assembler, (budget, limit)) = (self.session.assembler(), self.voicing());
        let job = Job {
            offset,
            dream: None,
            hush: Hush::default(),
            keeper: keeper.clone(),
            extraction,
            config,
            owner: owner.clone(),
            session: session.clone(),
            fallback: self.model.reference(),
            after,
            extractor: Arc::clone(extractor),
            // 放不下、接着抽剩下的：不等闲的那一次接着也不等闲（施工 E-5）。
            again: {
                let (extractor, backs) = (Arc::clone(extractor), self.backs.clone());
                Box::new(move || {
                    let generation = match forced {
                        true if !extractor.force() => return,
                        true => 0,
                        false => extractor.arm(),
                    };
                    answer_back(&backs, Back::ExtractDue { generation, forced });
                })
            },
        };
        // 拼历史、渲染、定怎么办挪进阻塞线程（施工 PERF-4）：大会话上要几毫秒，不占会话；定好了直接派出去。用的都是这一刻
        // 拿好的，同原来在 actor 上一口气做完的一样。
        let (runtime, span) = (tokio::runtime::Handle::current(), tracing::Span::current());
        tokio::task::spawn_blocking(move || {
            let plan = planned(
                events,
                after,
                &Planning {
                    assembler: assembler.as_ref(),
                    budget,
                    limit,
                    min_turns,
                    offset,
                    texts: &job.extraction.texts,
                    name: name.as_deref(),
                },
            );
            runtime.spawn(job.run(plan).instrument(span));
        });
    }
}

impl Actor {
    /// 人叫她现在就整理记忆（施工 R-7 补，`memory.md` 第七条第 9 款）：记忆开着的，在阻塞线程里读这一段（同闹钟响了的那一条
    /// 路，不看闲没闲够）；正忙的、读不了会话日志的不抽，直接合。记忆关着的当场交回用不了。
    pub(super) fn dream(&self, reply: oneshot::Sender<Result<Dreamed, NotDreamed>>) {
        let Some((calls, offset)) = self.tools.memory() else {
            return answer(reply, Err(NotDreamed::Off));
        };
        let (keeper, session, owner, extractor) = calls.extracting();
        // 从叫的这一刻起听着叫停（施工 R-22）：抽的时候叫停的，抽完不合。
        let hush = keeper.hush_for(extractor.hushes.listen());
        let config = Arc::clone(self.config.current());
        let dir = self.store.as_ref().and_then(|store| store.dir());
        let (Some(dir), true) = (dir, self.session.vacant()) else {
            let (keeper, owner) = (keeper.clone(), owner.clone());
            let dreaming = async move { keeper.dream_until(config, owner, offset, hush).await };
            tokio::spawn(async move { answer(reply, dreaming.await) });
            return;
        };
        let generation = extractor.arm();
        let floor = calls.floor();
        let (keeper, session, backs) = (keeper.clone(), session.clone(), self.backs.clone());
        tokio::task::spawn_blocking(move || {
            let read = keeper
                .extracted(&session)
                .map_err(|refused| format!("{refused:?}"))
                .and_then(|after| {
                    let events =
                        miyu_store::log::read_events(&dir).map_err(|error| error.to_string())?;
                    Ok((after.unwrap_or(Seq::FIRST).max(floor), events))
                });
            answer_back(
                &backs,
                Back::DreamRead {
                    generation,
                    read,
                    reply,
                    hush,
                },
            );
        });
    }

    /// 现在就整理的那一段读回来了：还闲着、没有一次在路上的，答了一轮就抽，抽完现在就合（`Job` 的 `dream`）；不然直接合。
    pub(super) fn dream_read(
        &self,
        generation: u64,
        read: Result<(Seq, Vec<Event>), String>,
        reply: oneshot::Sender<Result<Dreamed, NotDreamed>>,
        hush: Hush,
    ) {
        let Some((calls, offset)) = self.tools.memory() else {
            return answer(reply, Err(NotDreamed::Off));
        };
        let (keeper, session, owner, extractor) = calls.extracting();
        let config = Arc::clone(self.config.current());
        let extraction = keeper.extraction();
        let read = match (read, extraction) {
            (Ok(read), Some(extraction))
                if self.session.vacant() && extractor.start(generation) =>
            {
                Some((read, extraction))
            }
            (Err(error), _) => {
                tracing::warn!(target: crate::TARGET, error = error.as_str(), "memory extraction failed");
                None
            }
            _ => None,
        };
        let Some(((after, events), extraction)) = read else {
            extractor.disarm(generation);
            let (keeper, owner) = (keeper.clone(), owner.clone());
            let dreaming = async move { keeper.dream_until(config, owner, offset, hush).await };
            tokio::spawn(async move { answer(reply, dreaming.await) });
            return;
        };
        let name = self.owner_name(&config);
        let (budget, limit) = self.voicing();
        let assembler = self.session.assembler();
        let plan = planned(
            events,
            after,
            &Planning {
                assembler: assembler.as_ref(),
                budget,
                limit,
                min_turns: 1,
                offset,
                texts: &extraction.texts,
                name: name.as_deref(),
            },
        );
        let job = Job {
            offset,
            dream: Some(reply),
            hush,
            keeper: keeper.clone(),
            extraction,
            config,
            owner: owner.clone(),
            session: session.clone(),
            fallback: self.model.reference(),
            after,
            extractor: Arc::clone(extractor),
            again: Box::new(|| {}),
        };
        tokio::spawn(job.run(plan).in_current_span());
    }
}

mod planned;

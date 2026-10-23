//! 整理会话（施工 D-1 上，设计 `41-整理会话.md`）：记忆模块为一趟整理开的会话。会话表造它（裸的快照，没有人格、工具、记忆；
//! `session.created` 带 `dream`），经 [`DreamSessions`] 交给记忆：开一趟、一批一轮地跑（先清空，再以记忆模块的名义发这一批，
//! 等这一轮结束读回答）、叫停（打断那一轮）。端口拿着核心的弱引用：核心没了（正在退出）的，开不了、跑不了。

use std::sync::{Arc, Weak};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Dream, EndReason, Level, Permission, TransientBody};
use miyu_kernel::id::{CommandId, ModuleId, SessionId, TurnId, VenueId};
use miyu_kernel::origin::{By, Module};
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_policy::Profile;
use miyu_policy::memory::MemoryScope;
use miyu_session::{
    Create, DreamBatch, DreamRoom, DreamSessions, DreamSpec, Pending, Pushed, Watched, create,
    new_id,
};

use super::{Running, Sessions, environment, now};
use crate::Core;
use crate::{personas, spawn};
use stream::Stream;

/// 整理会话里说话的模块：记忆。
const MODULE: &str = "memory";

/// 记忆模块：整理会话的造的一方、每一批的说话的一方。
fn memory_module() -> Result<By, String> {
    let id = ModuleId::parse(MODULE).map_err(|error| error.to_string())?;
    Ok(By::Module(Module { id }))
}

impl Sessions {
    /// 造一个整理会话（施工 D-1 上）：裸的快照，没有人格、工具、记忆、知识库、技能、预设；没人看着；只读。由记忆模块造，
    /// `cause` 是 `<会话>/dream`。放进表里，和头造的一样照编号找得到。
    pub(crate) async fn dream(
        &self,
        core: &Arc<Core>,
        spec: DreamSpec,
    ) -> Result<SessionId, String> {
        let mut open = self.open.lock().await;
        let id = new_id(now());
        let by = memory_module()?;
        let command =
            CommandId::parse(&format!("{id}/dream")).map_err(|error| error.to_string())?;
        let (persona, session) = match spec.room {
            DreamRoom::Persona(persona) => (Some(persona), None),
            DreamRoom::Session(session) => (None, Some(session)),
        };
        let owner = spec.owner;
        let workspace = "~".to_string();
        let handle = create(Create {
            root: &core.root,
            resources: &core.resources,
            id: id.clone(),
            persona: None,
            persona_texts: Default::default(),
            profile: Profile::default(),
            personas: personas::personas(core, &owner),
            memory_account: core.memory_owner(&owner),
            memory_scope: MemoryScope::Off,
            venue: VenueId::parse(crate::list::LOCAL).map_err(|error| error.to_string())?,
            sandbox_cache: core.sandbox_cache_of(&owner),
            index: Some(core.index_for(&owner)),
            usage: core.usage_for(&owner),
            knowledge: None,
            vectors: None,
            installed: None,
            memory: None,
            configs: core.configs_of(&owner),
            owner_is_admin: owner == core.admin,
            owner,
            permission: Permission {
                level: Level::Workspace,
                read_only: true,
            },
            attended: false,
            oneshot: false,
            environment: environment(workspace.clone(), Vec::new()),
            command,
            by,
            models: &*core.models,
            tools: &core.tools,
            home: core.home.as_deref(),
            sandbox: core.sandbox.helper(),
            lineage: None,
            sessions: Some(spawn::port(core)),
            packages: None,
            hooks: None,
            jobs: &core.jobs,
            model: spec.model,
            preset: None,
            no_tools: false,
            presets: None,
            group: None,
            images: true,
            titles: true,
            dream: Some(Dream {
                persona,
                session,
                manual: spec.manual,
                from: spec.from,
            }),
            origin: None,
        })
        .await
        .map_err(|error| error.to_string())?;
        let running = Running {
            handle,
            workspace,
            dirs: Vec::new(),
        };
        open.running.insert(id.clone(), running);
        Ok(id)
    }
}

impl Core {
    /// 把整理会话交给记忆（施工 D-1 上）：核心套进 `Arc` 以后接一次。不接的（测试里）记忆照旧走一次性入口。交回接没接上。
    pub fn give_dream_sessions(self: &Arc<Self>) -> bool {
        let port = Arc::new(Table(Arc::downgrade(self)));
        self.memory.give_dream_sessions(port)
    }
}

/// 会话表那一头。
#[derive(Debug)]
struct Table(Weak<Core>);

impl Table {
    /// 核心还在。
    fn core(&self) -> Result<Arc<Core>, String> {
        self.0
            .upgrade()
            .ok_or_else(|| "the core is shutting down".to_string())
    }
}

impl DreamSessions for Table {
    fn open(&self, spec: DreamSpec) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async move {
            let core = self.core()?;
            core.sessions.dream(&core, spec).await
        })
    }

    fn batch<'a>(
        &'a self,
        session: SessionId,
        id: CommandId,
        text: String,
        watch: &'a mut (dyn FnMut(Watched<'_>) + Send),
    ) -> Pending<'a, Result<DreamBatch, String>> {
        Box::pin(async move {
            let core = self.core()?;
            let found = core
                .sessions
                .get(&core, &session)
                .await
                .map_err(|refusal| format!("session {session} not found: {refusal:?}"))?;
            let gone = || format!("session {session} stopped");
            // 先订阅：这一批的事件一定先于发话的回应到（`04-核心协议.md` 第六节第 2 条）。
            let mut pushes = found.handle.subscribe().await.map_err(|_| gone())?;
            let by = memory_module()?;
            // 批和批之间不带上下文（设计 41 DS2）：头一批前是空的，拒成「没有能清空的」不要紧。
            let clearing =
                CommandId::parse(&format!("{id}/clear")).map_err(|error| error.to_string())?;
            found
                .handle
                .command(clearing, by.clone(), Command::Clear)
                .await
                .map_err(|_| gone())?;
            let send = Command::Send {
                blocks: vec![Block::Text(Text { text })],
                urgent: false,
                venue: None,
            };
            let outcome = found
                .handle
                .command(id, by, send)
                .await
                .map_err(|_| gone())?;
            let Outcome::Accepted { events } = outcome else {
                return Err(format!("session {session} refused the batch: {outcome:?}"));
            };
            let said = events
                .first()
                .copied()
                .ok_or_else(|| "nothing was recorded".to_string())?;
            let mut turn: Option<TurnId> = None;
            let (mut answer, mut error) = (String::new(), None);
            let mut stream = Stream::default();
            loop {
                let pushed = pushes.next().await.map_err(|_| gone())?;
                let events = match &*pushed {
                    Pushed::Events(events) => events,
                    Pushed::Transient(transient) => {
                        if let TransientBody::ModelDelta(delta) = &transient.body
                            && turn.is_some()
                            && transient.turn == turn
                        {
                            stream.see(delta, watch);
                        }
                        continue;
                    }
                    Pushed::Goal { .. } => continue,
                };
                for event in events {
                    match &event.body {
                        Body::TurnStarted(started) if started.trigger == Some(said) => {
                            turn = event.turn;
                        }
                        _ if turn.is_none() || event.turn != turn => {}
                        Body::MessageAssistant(reply) => {
                            for block in &reply.blocks {
                                if let Block::Text(text) = block {
                                    answer.push_str(&text.text);
                                }
                            }
                        }
                        Body::ModelCalled(called) => {
                            if let Some(failed) = &called.error {
                                error = Some(failed.message.clone());
                            }
                        }
                        Body::TurnEnded(ended) => {
                            return Ok(match &ended.reason {
                                EndReason::Completed => DreamBatch::Answered(answer),
                                EndReason::Interrupted => DreamBatch::Stopped,
                                reason => DreamBatch::Failed(match error {
                                    Some(error) => format!("{}: {error}", reason.as_str()),
                                    None => reason.as_str().to_string(),
                                }),
                            });
                        }
                        _ => {}
                    }
                }
            }
        })
    }

    fn close(&self, session: SessionId, keep: usize) -> Pending<'_, Result<usize, String>> {
        Box::pin(async move {
            let core = self.core()?;
            Ok(keep::prune(&core, &session, keep).await)
        })
    }

    fn stop(&self, session: SessionId, id: CommandId) -> Pending<'_, Result<(), String>> {
        Box::pin(async move {
            let core = self.core()?;
            let found = core
                .sessions
                .get(&core, &session)
                .await
                .map_err(|refusal| format!("session {session} not found: {refusal:?}"))?;
            let interrupt = Command::Interrupt {
                queued: Queued::Return,
            };
            found
                .handle
                .command(id, memory_module()?, interrupt)
                .await
                .map(|_| ())
                .map_err(|_| format!("session {session} stopped"))
        })
    }
}

mod keep;
mod stream;

#[cfg(test)]
mod tests;

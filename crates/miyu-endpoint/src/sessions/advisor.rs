//! 顾问会话，会话表这一头（施工 ADV-1，设计 `69-旗舰顾问.md` 第三节，`docs/blueprint/tools/advisor.md`）：`advisor` 经会话端口
//! （`miyu_session::SessionPort::advise`）来问。旗舰池里没有可用模型的不造会话、不发请求；有的新开一个顾问会话（`continue` 的
//! 找父会话最近一次的那一个），把问题作为父会话发来的话送进去，等这一轮答完交回回答。
//!
//! 顾问会话是父会话的子会话（`session.created` 带 `parent`、`depth`，用量照子会话记、算进父会话的那一棵），`origin` 是
//! [`ADVISOR_ORIGIN`]：快照是裸的、system 换成顾问那一份（`miyu_session` 照 `origin` 拼），没有工具、没有人格，钉 `@flagship`。
//! 它不是父会话的任务：父会话的日志里没有它的 `job.started`，收空子会话时不收它（`orphans.rs`）。

use std::sync::{Arc, PoisonError, Weak};

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Queued};
use miyu_models::pools::{self, FLAGSHIP};
use miyu_policy::Profile;
use miyu_policy::memory::MemoryScope;
use miyu_session::{ADVISOR_ORIGIN, Advice, Create, create, new_id};
use miyu_tool::{Advised, NotAdvised};

use super::asked::{self, Turned};
use super::{Running, Sessions, environment, now};
use crate::Core;
use crate::{personas, spawn};

impl Sessions {
    /// 问一次顾问（设计 69 第三节）：照 `advice` 找、造顾问会话，送进问题，等它这一轮答完。
    ///
    /// # Errors
    ///
    /// 旗舰池里没有可用模型：[`NotAdvised::NoModel`]，什么都没造、没发。造不成会话、问不进去、这一轮出错、被打断：
    /// [`NotAdvised::Failed`]，英文的一句为什么。
    pub(crate) async fn advise(
        &self,
        core: &Arc<Core>,
        advice: Advice,
    ) -> Result<Advised, NotAdvised> {
        // 照这一刻的配置看池（`miyu_models::pools::pool`：成员一个都认不出的也算没有），不发请求就知道。
        let empty = {
            let config = core.config();
            pools::pool(&config.resolved().values(), FLAGSHIP).is_err()
        };
        if empty {
            return Err(NotAdvised::NoModel);
        }
        let parent = advice.lineage.parent.clone();
        let (ask, stop) = (
            command_id(&advice.command, "/ask")?,
            command_id(&advice.command, "/stop")?,
        );
        let continued = match advice.continued {
            true => latest(core, &parent).await,
            false => None,
        };
        // 同一个顾问会话一次只答一个问题：一步里并着调两次 `continue` 的，后一个排进前一个正在答的那一轮，会一直等不到自己的
        // 那一轮（`kernel/session.md`「排队的消息」）；占不到的照新开。
        let session = match continued.filter(|session| self.claim(session)) {
            Some(session) => session,
            None => {
                let session = self
                    .open_advisor(core, &advice)
                    .await
                    .map_err(NotAdvised::Failed)?;
                self.claim(&session);
                session
            }
        };
        // 交回之前放掉占着的；等到一半被掐掉的打断顾问那一轮，不白花旗舰模型的钱。
        let mut waiting = Waiting {
            core: Arc::downgrade(core),
            session: session.clone(),
            id: stop,
            by: By::Session(Session { id: parent.clone() }),
            answered: false,
        };
        let found = self.get(core, &session).await.map_err(|refusal| {
            NotAdvised::Failed(format!("session {session} not found: {refusal:?}"))
        })?;
        let by = By::Session(Session { id: parent });
        let turned = asked::ask(
            (&found.handle, &session),
            ask,
            by,
            advice.question,
            &mut |_| {},
        )
        .await
        .map_err(NotAdvised::Failed)?;
        waiting.answered = true;
        match turned {
            Turned::Answered(answer) => Ok(Advised { session, answer }),
            Turned::Stopped => Err(NotAdvised::Failed("interrupted".to_string())),
            Turned::Failed(why) => Err(NotAdvised::Failed(why)),
        }
    }

    /// 占着顾问会话 `session`：交回占没占上（别的调用正占着的占不上）。
    fn claim(&self, session: &SessionId) -> bool {
        self.answering
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(session.clone())
    }

    /// 新开一个顾问会话：父会话造（`by` 是它），场所、权限、工作目录、显示图片、关不关照 `advice`；无人格、无工具、钉
    /// `@flagship`，`origin` 是顾问；记忆、知识库、技能、预设、扩展都交成关着的。放进表里，和子代理一样照编号找得到。
    async fn open_advisor(&self, core: &Arc<Core>, advice: &Advice) -> Result<SessionId, String> {
        let mut open = self.open.lock().await;
        // 父会话被删了：不再造，不留下没有父会话的子会话（同子代理）。
        if !open.running.contains_key(&advice.lineage.parent) {
            return Err("the parent session is gone".to_string());
        }
        let id = new_id(now());
        let by = By::Session(Session {
            id: advice.lineage.parent.clone(),
        });
        let resources = crate::presets::overrides::resources(core);
        let handle = create(Create {
            root: &core.root,
            resources: &resources,
            id: id.clone(),
            persona: None,
            persona_texts: Default::default(),
            profile: Profile::default(),
            personas: personas::personas(core),
            memory_scope: MemoryScope::Off,
            venue: advice.venue.clone(),
            index: Some(core.index()),
            usage: core.ledger(),
            knowledge: None,
            vectors: None,
            installed: None,
            plugins: core.plugins.clone(),
            memory: None,
            configs: core.configs_of(),
            permission: advice.permission.clone(),
            attended: false,
            oneshot: false,
            environment: environment(advice.cwd.clone(), advice.dirs.clone()),
            command: advice.command.clone(),
            by,
            models: &*core.models,
            tools: core.tools.clone(),
            home: core.home.as_deref(),
            sandbox: core.sandbox.helper(),
            lineage: Some(advice.lineage.clone()),
            sessions: Some(spawn::port(core)),
            packages: None,
            schedules: None,
            hooks: None,
            jobs: &core.jobs,
            model: Some(format!("@{FLAGSHIP}")),
            preset: None,
            no_tools: true,
            presets: None,
            group: None,
            confined: advice.confined,
            images: advice.images,
            titles: false,
            dream: None,
            origin: Some(ADVISOR_ORIGIN.to_string()),
        })
        .await
        .map_err(|error| error.to_string())?;
        let running = Running {
            handle,
            workspace: advice.cwd.clone(),
            dirs: advice.dirs.clone(),
        };
        open.running.insert(id.clone(), running);
        Ok(id)
    }
}

/// 父会话 `parent` 最近一次的顾问会话：照会话列表的索引，`parent` 是它、`origin` 是顾问的，造得最晚的那一个。读不出索引的、
/// 一个都没有的没有（照新开）。
async fn latest(core: &Core, parent: &SessionId) -> Option<SessionId> {
    let (index, parent) = (core.index(), parent.clone());
    let rows = tokio::task::spawn_blocking(move || index.rows()).await;
    let rows = match rows {
        Ok(Ok(rows)) => rows,
        Ok(Err(error)) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "advisor sessions not looked up");
            return None;
        }
        Err(error) => {
            tracing::error!(target: "miyu::endpoint", panic = error.is_panic(), "advisor lookup panicked");
            return None;
        }
    };
    rows.into_values()
        .filter(|row| {
            row.parent.as_ref() == Some(&parent) && row.origin.as_deref() == Some(ADVISOR_ORIGIN)
        })
        .max_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)))
        .map(|row| row.id)
}

/// 这次问顾问的命令编号 `command` 后面接 `suffix`。
fn command_id(command: &CommandId, suffix: &str) -> Result<CommandId, NotAdvised> {
    CommandId::parse(&format!("{command}{suffix}"))
        .map_err(|error| NotAdvised::Failed(error.to_string()))
}

/// 在等顾问会话 `session` 答：丢掉时放掉占着的它；还没答完的（调用被掐掉、出错）在别的任务里打断它那一轮，不等。核心正在
/// 停、没有运行时的不发。
struct Waiting {
    core: Weak<Core>,
    session: SessionId,
    id: CommandId,
    by: By,
    answered: bool,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let Some(core) = self.core.upgrade() else {
            return;
        };
        core.sessions
            .answering
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.session);
        if self.answered {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (session, id, by) = (self.session.clone(), self.id.clone(), self.by.clone());
        runtime.spawn(async move {
            let Ok(found) = core.sessions.get(&core, &session).await else {
                return;
            };
            let interrupt = Command::Interrupt {
                queued: Queued::Return,
            };
            if found.handle.command(id, by, interrupt).await.is_err() {
                tracing::debug!(target: "miyu::endpoint", session = session.as_str(), "advisor already stopped");
            }
        });
    }
}

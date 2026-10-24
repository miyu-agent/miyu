//! 一次调用的派子代理的端口（施工 7-5；施工 C-9 从 `agents.rs` 挪出来）：领编号、造子会话、把交代连同附件送进去。fork 子代理
//! （施工 A-F 中，`agents.md`「fork 子代理」）经会话表从这次调用那一步分叉，交代前面接上外壳。

use std::sync::Arc;

use miyu_kernel::event::Permission;
use miyu_kernel::id::{CallId, JobId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome};
use miyu_store::blob::Blobs;
use miyu_tool::{AgentPort, NotSpawned, Order, Spawned, Spawning};

use super::{Agents, Inherit, JOB_DEPTH, command_id};
use crate::TARGET;
use crate::attached::{blocks, kept_for};
use crate::job_ids::JobIds;
use crate::spawn::{Child, ForkChild, Lineage};

/// 一次调用的派子代理的端口。
pub(super) struct Spawner {
    pub(super) agents: Arc<Agents>,
    /// 这次调用：fork 子代理从它那一步分叉（施工 A-F 中）。
    pub(super) call: CallId,
    pub(super) ids: Arc<JobIds>,
    pub(super) cwd: String,
    pub(super) dirs: Vec<String>,
    pub(super) permission: Permission,
    pub(super) inherit: Inherit,
    /// 这个会话的 blob：交代带的附件存进去（施工 C-9）。
    pub(super) blobs: Blobs,
}

impl AgentPort for Spawner {
    /// 标题不交给子会话：它只给头看，记在 `job.started` 里（工具报）。人格没挑的是无人格（施工 P-4 上：出厂不带人格，原来是
    /// 软件工程师，走查 C5），预设照父会话的。
    fn spawn<'a>(&'a self, order: Order<'a>) -> Spawning<'a> {
        let Order {
            prompt,
            pool,
            persona,
            attached,
            fork,
            ..
        } = order;
        Box::pin(async move {
            let job = self.ids.next();
            let agents = &self.agents;
            let parent = &agents.session;
            let job_text = job.to_string();
            // 深度照算（施工 A-F 中）：fork 子代理的工具面照父会话的，到了上限也有 `subagent`，派的时候挡。
            if agents.depth + 1 > JOB_DEPTH {
                tracing::warn!(target: TARGET, job = job_text.as_str(), "subagent too deep");
                return Err(NotSpawned);
            }
            let prompt = match fork {
                true => self.directed(prompt).map_err(|error| {
                    tracing::warn!(target: TARGET, job = job_text.as_str(), error = error.as_str(), "fork directive not read");
                    NotSpawned
                })?,
                false => prompt.to_string(),
            };
            let child = Child {
                lineage: Lineage {
                    parent: parent.clone(),
                    depth: agents.depth + 1,
                    workspace_from: agents.workspace_from.clone(),
                },
                command: command_id(parent, &job, ""),
                persona: persona.map(str::to_string),
                owner: agents.owner.clone(),
                venue: agents.venue.clone(),
                permission: self.permission.clone(),
                attended: agents.attended,
                cwd: self.cwd.clone(),
                dirs: self.dirs.clone(),
                model: self.inherit.model(pool),
                preset: agents.preset.clone(),
                images: agents.images,
            };
            // 带的附件接在交代后面（施工 C-9）：先存好，存不下的不造子会话。
            let blocks = blocks(&self.blobs, &prompt, attached).await.map_err(|error| {
                tracing::warn!(target: TARGET, job = job_text.as_str(), error = error.as_str(), "subagent attachments not stored");
                NotSpawned
            })?;
            let made = match fork {
                true => self.forked(child).await,
                false => agents.port.create(child).await,
            };
            let session = made.map_err(|error| {
                tracing::warn!(target: TARGET, job = job_text.as_str(), error = error.as_str(), "subagent not created");
                NotSpawned
            })?;
            // 造好了、还没交回：这时被掐掉（打断、来了一句话），守着的停掉它（施工 7-5 补）。
            let mut unclaimed = Unclaimed {
                agents: Arc::clone(agents),
                job: job.clone(),
                child: Some(session.clone()),
            };
            // 子会话读得到交代带的附件（施工 3-9 六补）：落进它自己的附件目录。
            let blocks = kept_for(&agents.root, &agents.owner, &session, &self.blobs, blocks)
                .await
                .map_err(|error| {
                    tracing::warn!(target: TARGET, job = job_text.as_str(), error = error.as_str(), "subagent attachments not kept");
                    NotSpawned
                })?;
            // 交代原样送进去，不加包装：子会话的场所说明已经说了它来自父会话（第一条第 2 条）。
            let by = By::Session(Session { id: parent.clone() });
            let send = Command::Send {
                blocks,
                urgent: false,
                venue: None,
            };
            let sent = agents
                .port
                .command(
                    session.clone(),
                    command_id(parent, &job, "/prompt"),
                    by,
                    send,
                )
                .await;
            let why = match sent {
                Ok(Outcome::Accepted { .. } | Outcome::Recapped { .. }) => {
                    tracing::info!(target: TARGET, job = job_text.as_str(), child = session.as_str(), "subagent started");
                    unclaimed.child = None;
                    return Ok(Spawned { job, session });
                }
                Ok(Outcome::Rejected { reason, .. }) => {
                    format!("the task was refused: {}", reason.code())
                }
                Err(error) => error,
            };
            tracing::warn!(target: TARGET, job = job_text.as_str(), child = session.as_str(), error = why.as_str(), "subagent not given its task");
            Err(NotSpawned)
        })
    }

    fn pools(&self) -> &[String] {
        &self.agents.pools
    }

    fn personas(&self) -> &[String] {
        &self.agents.personas
    }

    fn forked(&self) -> bool {
        self.agents.forked
    }
}

impl Spawner {
    /// fork 子代理收到的交代（施工 A-F 中）：外壳（`core/jobs/fork-directive.txt`，去掉末尾的空白）、一个空行、她写的交代。
    fn directed(&self, prompt: &str) -> Result<String, String> {
        let shell = self
            .agents
            .resources
            .fork_directive()
            .map_err(|error| error.to_string())?;
        Ok(format!("{}\n\n{prompt}", shell.trim_end()))
    }

    /// 经会话表从这次调用那一步分叉出子会话（施工 A-F 中）：属主、父会话、层数、命令编号照 `child`，别的照父会话落了盘的日志。
    async fn forked(&self, child: Child) -> Result<SessionId, String> {
        let placeholder = self
            .agents
            .resources
            .fork_placeholder()
            .map_err(|error| error.to_string())?;
        let fork = ForkChild {
            lineage: child.lineage,
            command: child.command,
            owner: child.owner,
            call: self.call,
            placeholder,
        };
        self.agents.port.fork(fork).await
    }
}

/// 造好了、还没交回的子会话（施工 7-5 补，`agents.md` 第一条第 9 条）：派到一半被掐掉的，丢掉时停掉它，不留没人管的
/// 子代理；交回了就不管（`child` 拿走）。
struct Unclaimed {
    agents: Arc<Agents>,
    job: JobId,
    child: Option<SessionId>,
}

impl Drop for Unclaimed {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            self.agents.stop_unclaimed(&self.job, child);
        }
    }
}

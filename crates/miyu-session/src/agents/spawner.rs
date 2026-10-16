//! 一次调用的派子代理的端口（施工 7-5；施工 C-9 从 `agents.rs` 挪出来）：领编号、造子会话、把交代连同附件送进去。

use std::sync::Arc;

use miyu_kernel::event::Permission;
use miyu_kernel::id::{JobId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome};
use miyu_store::blob::Blobs;
use miyu_tool::{AgentPort, NotSpawned, Order, Spawned, Spawning};

use super::{Agents, Inherit, command_id};
use crate::TARGET;
use crate::attached::{blocks, kept_for};
use crate::job_ids::JobIds;
use crate::spawn::{Child, Lineage};

/// 一次调用的派子代理的端口。
pub(super) struct Spawner {
    pub(super) agents: Arc<Agents>,
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
            ..
        } = order;
        Box::pin(async move {
            let job = self.ids.next();
            let agents = &self.agents;
            let parent = &agents.session;
            let child = Child {
                lineage: Lineage {
                    parent: parent.clone(),
                    depth: agents.depth + 1,
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
            let job_text = job.to_string();
            // 带的附件接在交代后面（施工 C-9）：先存好，存不下的不造子会话。
            let blocks = blocks(&self.blobs, prompt, attached).await.map_err(|error| {
                tracing::warn!(target: TARGET, job = job_text.as_str(), error = error.as_str(), "subagent attachments not stored");
                NotSpawned
            })?;
            let session = agents.port.create(child).await.map_err(|error| {
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

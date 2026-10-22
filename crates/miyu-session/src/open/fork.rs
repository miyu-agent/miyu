//! 分叉（施工 C-8 上，`docs/blueprint/session/fork.md`）：照父会话落了盘的日志算分叉点和那一刻的权限、模型、快照，写好子会话
//! 的第一条（带 `forked` 的 `session.created`，序号接着分叉点），交回分叉点。之后由调的一方照常载入它（载入认得 `forked`，
//! 照它读父会话的日志当底子）。

use std::fmt;
use std::io;
use std::num::NonZeroU32;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Forked, SessionCreated};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, CommandId, Seq, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Action, LoadError as Broken, Session, fork_plan};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_policy::{BuildError, Snapshot, SnapshotError};
use miyu_store::blob::{BlobError, Blobs};
use miyu_store::index::SessionIndex;
use miyu_store::log::{OpenError, SEGMENT_LIMIT, SessionLog, abandon, read_events};
use miyu_store::root::DataRoot;

use super::scan::remembered;
use crate::blocking::blocking;
use crate::store::{Indexed, Store};

/// 分叉要的几样。
pub struct Fork<'a> {
    /// 数据根。
    pub root: &'a DataRoot,
    /// 父会话的属主：子会话照它。
    pub owner: &'a AccountId,
    /// 父会话。
    pub parent: &'a SessionId,
    /// 子会话的编号。
    pub id: SessionId,
    /// 分叉的命令编号：子会话的 `session.created` 的 `cause`。
    pub cause: CommandId,
    /// 谁分叉的。
    pub by: By,
    /// 此刻。
    pub at: Timestamp,
    /// 会话列表的索引：子会话的第一条照样记进去（同造会话）。没有的不记。
    pub index: Option<Arc<SessionIndex>>,
    /// 跑完几轮封存（施工 C-8 下）：写进 `forked`。不封存的没有。
    pub seal_after: Option<NonZeroU32>,
}

/// 分叉不成。
#[derive(Debug)]
pub enum ForkError {
    /// 父会话的日志打不开：没有这个会话，或者日志坏了。
    Log(OpenError),
    /// 父会话自己是子会话或者分叉出来的：只能从主会话分叉。
    NotMain,
    /// 父会话的日志过不了账本、分叉点算不出来。
    Kernel(Broken),
    /// 父会话的策略快照取不出来、读不懂、造不出策略。
    Blob(BlobError),
    /// 同上，读不懂。
    Snapshot(SnapshotError),
    /// 同上，造不出策略。
    Policy(BuildError),
    /// 建不了子会话的目录和日志、写不进第一条。
    Disk(io::Error),
}

impl fmt::Display for ForkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForkError::Log(error) => write!(f, "parent log not read: {error}"),
            ForkError::NotMain => write!(f, "only a main session can be forked"),
            ForkError::Kernel(error) => write!(f, "fork point not found: {error}"),
            ForkError::Blob(error) => write!(f, "policy snapshot not fetched: {error}"),
            ForkError::Snapshot(error) => write!(f, "policy snapshot not understood: {error}"),
            ForkError::Policy(error) => write!(f, "policy not built from the snapshot: {error}"),
            ForkError::Disk(error) => write!(f, "fork not written: {error}"),
        }
    }
}

impl std::error::Error for ForkError {}

/// 分叉：写好子会话的第一条，交回分叉点。父会话照只读读（它可能正在写）。
///
/// # Errors
///
/// 见 [`ForkError`]。
pub async fn fork(setup: Fork<'_>) -> Result<Seq, ForkError> {
    let Fork {
        root,
        owner,
        parent,
        id,
        cause,
        by,
        at,
        index,
        seal_after,
    } = setup;
    let parent_dir = root.session_dir(owner, parent);
    let child_dir = root.session_dir(owner, &id);
    let blobs = Blobs::new(root.blobs(owner));
    let parent = parent.clone();
    blocking(move || {
        let events = read_events(&parent_dir).map_err(ForkError::Log)?;
        let created = match events.first().map(|event| &event.body) {
            Some(Body::SessionCreated(created)) => created.clone(),
            _ => return Err(ForkError::Kernel(Broken::Empty)),
        };
        // 子代理、分叉出来的都有父会话。
        if created.parent.is_some() {
            return Err(ForkError::NotMain);
        }
        let plan = fork_plan(&parent, &events).map_err(ForkError::Kernel)?;
        let base: Vec<Event> = events
            .into_iter()
            .take_while(|event| event.seq <= plan.upto)
            .collect();
        let bytes = blobs.get(&plan.policy).map_err(ForkError::Blob)?;
        let snapshot = Snapshot::from_bytes(&bytes).map_err(ForkError::Snapshot)?;
        let policy = snapshot.policy().map_err(ForkError::Policy)?;
        let (cwd, dirs) = remembered(&base);
        let child = SessionCreated {
            owner: created.owner,
            venue: created.venue,
            policy: plan.policy,
            permission: plan.permission,
            oneshot: false,
            cwd: cwd.clone().or(created.cwd),
            parent: Some(parent.clone()),
            depth: Some(1),
            workspace_from: created.workspace_from.or_else(|| Some(parent.clone())),
            model: plan.model,
            persona: plan.persona,
            preset: plan.preset.or(created.preset),
            // 无工具的会话分叉出来的照旧无工具（施工 P-16 下）：快照是同一份。
            no_tools: created.no_tools,
            images: created.images,
            titles: created.titles,
            // 分叉出来的是普通会话，不算整理会话（施工 D-1 上）。
            dream: None,
            origin: None,
            forked: Some(Forked {
                from: parent,
                upto: plan.upto,
                seal_after,
            }),
        };
        // 这一份内核会话只用来查底子、写第一条：环境照记下的，时区不进第一条。
        let environment = Environment {
            offset: UtcOffset::UTC,
            cwd: cwd.unwrap_or_default(),
            dirs,
        };
        let first_id = id.clone();
        let (_, actions) = Session::fork(id, &base, cause, by, at, child, policy, environment)
            .map_err(ForkError::Kernel)?;
        let first: Vec<Event> = actions
            .into_iter()
            .filter_map(|action| match action {
                Action::Append(events) => Some(events),
                _ => None,
            })
            .flatten()
            .collect();
        let start = plan.upto.next();
        let log =
            SessionLog::create_at(&child_dir, SEGMENT_LIMIT, start).map_err(ForkError::Disk)?;
        let mut store = Indexed::new(log, &first_id, index, None, None);
        if let Err(error) = store.append(&first) {
            drop(store);
            if let Err(cleaning) = abandon(&child_dir) {
                tracing::warn!(target: crate::TARGET, error = %cleaning, "fork dir not cleaned");
            }
            return Err(ForkError::Disk(error));
        }
        Ok(plan.upto)
    })
    .await
}

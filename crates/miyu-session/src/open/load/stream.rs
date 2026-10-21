//! 读日志接着重放（施工 V-15，`docs/construction/V-15-重开大会话边读边重放.md`）：日志一块块读，头一块认出造会话那一条、
//! 父会话的底子、这时的快照，接着一块块交给内核过账本（`Loader`），载入要的几样跟着记（`Scan`）；读过的放掉，整份不同时
//! 留在内存里。
//!
//! 读完才认得出最近一次换的快照：和头一块认的不是同一份的，照最后那一份取策略；内核说一块块喂的对不上（压缩被撤到放掉的
//! 前面、向上回报的截法换了），照整份再读一遍，走以前的 `Session::load`。

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use miyu_kernel::event::{Body, Event, SessionCreated};
use miyu_kernel::id::{AccountId, CommandId, ContentHash, SessionId};
use miyu_kernel::session::{Loader, Policy, Session};
use miyu_policy::Snapshot;
use miyu_store::blob::Blobs;
use miyu_store::log::{Flow, SEGMENT_LIMIT, SessionLog, read_events};
use miyu_store::root::DataRoot;

use super::LoadError;
use crate::open::scan::{Scan, current_policy};

/// 读完了的日志。
pub(super) struct Read {
    /// 开着的日志，接着往里写。
    pub(super) log: SessionLog,
    /// 造会话那一条，和它的 `cause`。
    pub(super) created: (SessionCreated, Option<CommandId>),
    /// 分叉出来的：父会话的日志里到分叉点为止的。
    pub(super) base: Option<Arc<Vec<Event>>>,
    /// 现在的快照，和照它取的策略。
    pub(super) snapshot: (Snapshot, Policy),
    /// 内核那一头。
    pub(super) loaded: Loaded,
    /// 跟着记的几样。
    pub(super) scan: Scan,
    /// 取快照用了多久（`loaded` 那一行另记）。
    pub(super) snapshotted: Duration,
}

/// 内核那一头。
pub(super) enum Loaded {
    /// 一块块喂完了，等收尾。
    Fed(Box<Loader>),
    /// 一块块喂的对不上：整份，照以前的载入。
    Whole(Vec<Event>),
}

/// 照会话日志目录 `dir` 读、接着重放：会话 `id`，快照从 `store` 取，分叉出来的从数据根 `parents` 里账号 `owner` 的父会话
/// 读底子。
///
/// # Errors
///
/// 日志打不开、坏了；头一条不是造会话那一条；快照取不出、读不懂、造不出策略；有一条过不了账本。
pub(super) fn read(
    dir: &Path,
    id: &SessionId,
    store: &Blobs,
    (parents, owner): (&DataRoot, &AccountId),
) -> Result<Read, LoadError> {
    let mut started: Option<Started> = None;
    let mut failed = None;
    let mut scan = Scan::default();
    let mut snapshotted = Duration::ZERO;
    let opened = SessionLog::open_each(dir, SEGMENT_LIMIT, |events| {
        if started.is_none() {
            let at = Instant::now();
            match start(&events, id, store, (parents, owner)) {
                Ok(start) => {
                    if let Some(base) = &start.base {
                        scan.base(base);
                    }
                    started = Some(start);
                }
                Err(error) => {
                    failed = Some(error);
                    return Flow::Stop;
                }
            }
            snapshotted += at.elapsed();
        }
        scan.note(&events);
        let Some(start) = started.as_mut() else {
            return Flow::Stop;
        };
        match start.loader.feed(events) {
            Ok(()) => Flow::Go,
            Err(error) => {
                failed = Some(LoadError::Kernel(error));
                Flow::Stop
            }
        }
    });
    if let Some(error) = failed {
        return Err(error);
    }
    let log = opened.map_err(LoadError::Log)?;
    let Some(Started {
        created,
        base,
        hash,
        mut snapshot,
        loader,
    }) = started
    else {
        return Err(LoadError::NotCreated);
    };
    // 读完才认得出最近一次换的快照（同 `current_policy`）。
    if let Some(now) = scan.switched(&hash, &created.0.policy) {
        let at = Instant::now();
        snapshot = policy_of(store, &now)?;
        snapshotted += at.elapsed();
    }
    let loaded = if loader.needs_whole(&snapshot.1) {
        Loaded::Whole(read_events(dir).map_err(LoadError::Log)?)
    } else {
        Loaded::Fed(Box::new(loader))
    };
    Ok(Read {
        log,
        created,
        base,
        snapshot,
        loaded,
        scan,
        snapshotted,
    })
}

/// 头一块认出来的。
struct Started {
    created: (SessionCreated, Option<CommandId>),
    base: Option<Arc<Vec<Event>>>,
    /// 照着取策略的那一份快照。
    hash: ContentHash,
    snapshot: (Snapshot, Policy),
    loader: Loader,
}

/// 照头一块 `first` 认出造会话那一条、底子、这时的快照，开始一块块载入。
fn start(
    first: &[Event],
    id: &SessionId,
    store: &Blobs,
    (parents, owner): (&DataRoot, &AccountId),
) -> Result<Started, LoadError> {
    let created = match first.first() {
        Some(Event {
            body: Body::SessionCreated(created),
            cause,
            ..
        }) => (created.clone(), cause.clone()),
        _ => return Err(LoadError::NotCreated),
    };
    // 分叉出来的（施工 C-8 上）：父会话的日志里到分叉点为止的是底子，照只读读（父会话可能正在写）。
    let base = match &created.0.forked {
        Some(forked) => {
            let parent_log =
                read_events(&parents.session_dir(owner, &forked.from)).map_err(LoadError::Log)?;
            let base: Vec<Event> = parent_log
                .into_iter()
                .take_while(|event| event.seq <= forked.upto)
                .collect();
            Some(Arc::new(base))
        }
        None => None,
    };
    let hash = current_policy(first).unwrap_or(&created.0.policy).clone();
    let snapshot = policy_of(store, &hash)?;
    let loader = Session::loader(
        id.clone(),
        base.as_deref().cloned(),
        first,
        snapshot.1.reports.clone(),
    )
    .map_err(LoadError::Kernel)?;
    Ok(Started {
        created,
        base,
        hash,
        snapshot,
        loader,
    })
}

/// 取快照 `hash`、照它造策略。
fn policy_of(store: &Blobs, hash: &ContentHash) -> Result<(Snapshot, Policy), LoadError> {
    let bytes = store.get(hash).map_err(LoadError::Blob)?;
    let snapshot = Snapshot::from_bytes(&bytes).map_err(LoadError::Snapshot)?;
    let policy = snapshot.policy().map_err(LoadError::Policy)?;
    Ok((snapshot, policy))
}

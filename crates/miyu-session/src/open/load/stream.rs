//! 读日志接着重放（施工 V-15，`docs/construction/V-15-重开大会话边读边重放.md`）：日志一块块读，头一块认出造会话那一条、
//! 父会话的底子、这时的快照，接着一块块交给内核过账本（`Loader`），载入要的几样跟着记（`Scan`）；读过的放掉，整份不同时
//! 留在内存里。
//!
//! 读完才认得出最近一次换的快照：和头一块认的不是同一份的，照最后那一份取策略；内核说一块块喂的对不上（压缩被撤到放掉的
//! 前面、向上回报的截法换了），照整份再读一遍，走以前的 `Session::load`。
//!
//! 分叉出来的会话的底子也一块块读、一块块喂（施工 V-39 中，[`BaseFeed`]）：父会话压缩以前的那一截喂的时候就放掉，手上只留
//! 最近一次压缩以后到分叉点的；以前整份读进来交给内核，第一块自己的日志进来才放。
//!
//! 刚分叉出来、紧接着载入的支线（施工 V-39 下）：分叉那一遍喂好的底子交过来（[`ForkedBase`]），对得上这条支线、这个分叉点的
//! 直接用，连同底子里她看过的文件、回合索引认好的底子，不再读父会话；对不上、没交的照上面自己读。

use std::path::Path;
use std::time::{Duration, Instant};

use miyu_kernel::event::{Body, Event, SessionCreated};
use miyu_kernel::id::{CommandId, ContentHash, Seq, SessionId};
use miyu_kernel::session::{BaseFeed, Loader, Policy, Session};
use miyu_policy::Snapshot;
use miyu_recall::Priming;
use miyu_store::blob::Blobs;
use miyu_store::log::{Flow, SEGMENT_LIMIT, SessionLog, read_events};
use miyu_store::root::DataRoot;

use super::LoadError;
use crate::open::ForkedBase;
use crate::open::scan::{Scan, current_policy};
use crate::store::Base;

/// 读完了的日志。
pub(in crate::open) struct Read {
    /// 开着的日志，接着往里写。
    pub(in crate::open) log: SessionLog,
    /// 造会话那一条，和它的 `cause`。
    pub(in crate::open) created: (SessionCreated, Option<CommandId>),
    /// 分叉出来的：底子在哪（施工 V-20：只记位置，事件交给内核以后不留）。
    pub(in crate::open) base: Option<Base>,
    /// 现在的快照，和照它取的策略。
    pub(in crate::open) snapshot: (Snapshot, Policy),
    /// 内核那一头。
    pub(in crate::open) loaded: Loaded,
    /// 跟着记的几样。
    pub(in crate::open) scan: Scan,
    /// 分叉时回合索引已经照底子认好的（施工 V-39 下）：交过来、用上了的才有。
    pub(in crate::open) primed: Option<Priming>,
    /// 取快照用了多久（`loaded` 那一行另记）。
    pub(in crate::open) snapshotted: Duration,
}

/// 内核那一头。
pub(in crate::open) enum Loaded {
    /// 一块块喂完了，等收尾。
    Fed(Box<Loader>),
    /// 一块块喂的对不上：整份，照以前的载入。分叉出来的连底子一起重读。
    Whole {
        /// 底子：不是分叉出来的没有。
        base: Option<Vec<Event>>,
        /// 会话自己的整份日志。
        events: Vec<Event>,
    },
}

/// 照会话日志目录 `dir` 读、接着重放：会话 `id`，快照从 `store` 取，分叉出来的从数据根 `parents` 里的父会话
/// 读底子；分叉时读好交过来的（`handed`，施工 V-39 下）对得上的不读。
///
/// # Errors
///
/// 日志打不开、坏了；头一条不是造会话那一条；快照取不出、读不懂、造不出策略；有一条过不了账本。
pub(in crate::open) fn read(
    dir: &Path,
    id: &SessionId,
    store: &Blobs,
    parents: &DataRoot,
    mut handed: Option<ForkedBase>,
) -> Result<Read, LoadError> {
    let mut started: Option<Started> = None;
    let mut failed = None;
    let mut scan = Scan::default();
    let mut snapshotted = Duration::ZERO;
    let opened = SessionLog::open_each(dir, SEGMENT_LIMIT, |events| {
        if started.is_none() {
            let at = Instant::now();
            match start(&events, id, store, parents, &mut scan, handed.take()) {
                Ok(start) => started = Some(start),
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
        primed,
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
        Loaded::Whole {
            base: base
                .as_ref()
                .map(|base| base.events_from(Seq::FIRST))
                .transpose()
                .map_err(LoadError::Log)?,
            events: read_events(dir).map_err(LoadError::Log)?,
        }
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
        primed,
        snapshotted,
    })
}

/// 头一块认出来的。
struct Started {
    created: (SessionCreated, Option<CommandId>),
    base: Option<Base>,
    /// 照着取策略的那一份快照。
    hash: ContentHash,
    snapshot: (Snapshot, Policy),
    loader: Loader,
    primed: Option<Priming>,
}

/// 照头一块 `first` 认出造会话那一条、底子、这时的快照，开始一块块载入；底子里要记的交给 `scan` 记下。分叉时读好交过来的
/// 底子（`handed`）是这条支线、这个分叉点的，照它接着走，不读父会话；不是的放掉，照旧读。
fn start(
    first: &[Event],
    id: &SessionId,
    store: &Blobs,
    parents: &DataRoot,
    scan: &mut Scan,
    handed: Option<ForkedBase>,
) -> Result<Started, LoadError> {
    let created = match first.first() {
        Some(Event {
            body: Body::SessionCreated(created),
            cause,
            ..
        }) => (created.clone(), cause.clone()),
        _ => return Err(LoadError::NotCreated),
    };
    // 分叉出来的（施工 C-8 上）：父会话的日志里到分叉点为止的是底子，照只读读（父会话可能正在写）。一块块喂给内核（施工
    // V-39 中），这里只留它在哪（施工 V-20）：以前另留一份、再拷一份给内核，支线活多久底子留多久。
    let mut feed = None;
    let mut primed = None;
    let base = match &created.0.forked {
        Some(forked) => {
            let base = Base::new(parents.session_dir(&forked.from), forked.upto);
            // 分叉那一遍喂好的（施工 V-39 下）：底子到分叉点为止不变（父会话只往后写），喂的是同样的事件、同样的代码。
            match handed.filter(|handed| {
                handed.child == *id && handed.parent == forked.from && handed.upto == forked.upto
            }) {
                Some(handed) => {
                    scan.based(handed.seen);
                    feed = Some(handed.base);
                    primed = Some(handed.priming);
                }
                None => {
                    let mut fed = BaseFeed::new(forked.from.clone());
                    base.each(|events| {
                        scan.base(&events);
                        fed.feed(events);
                    })
                    .map_err(LoadError::Log)?;
                    feed = Some(fed);
                }
            }
            Some(base)
        }
        None => None,
    };
    let hash = current_policy(first).unwrap_or(&created.0.policy).clone();
    let snapshot = policy_of(store, &hash)?;
    let loader = Session::loader(id.clone(), feed, first, snapshot.1.reports.clone())
        .map_err(LoadError::Kernel)?;
    Ok(Started {
        created,
        base,
        hash,
        snapshot,
        loader,
        primed,
    })
}

/// 取快照 `hash`、照它造策略。
fn policy_of(store: &Blobs, hash: &ContentHash) -> Result<(Snapshot, Policy), LoadError> {
    let bytes = store.get(hash).map_err(LoadError::Blob)?;
    let snapshot = Snapshot::from_bytes(&bytes).map_err(LoadError::Snapshot)?;
    let policy = snapshot.policy().map_err(LoadError::Policy)?;
    Ok((snapshot, policy))
}

/// 毫秒，一位小数：运行日志里写载入各段用了多久（施工 V-2 中；V-39 下从 `load.rs` 挪来，那一份到了行数上限）。
pub(super) fn millis(took: Duration) -> f64 {
    (took.as_secs_f64() * 10_000.0).round() / 10.0
}

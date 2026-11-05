//! 分叉（施工 C-8 上，`docs/blueprint/session/fork.md`）：照父会话落了盘的日志算分叉点和那一刻的权限、模型、快照，写好子会话
//! 的第一条（带 `forked` 的 `session.created`，序号接着分叉点），交回分叉点。之后由调的一方照常载入它（载入认得 `forked`，
//! 照它读父会话的日志当底子）。fork 子代理（施工 A-F 中，[`Within`]）从父会话这一轮派它的那一步分叉，头几条多记那一步的
//! 占位结果、收住那一轮。
//!
//! 父会话一块块读（施工 V-39 中，`docs/construction/V-39中-支线底子边读边喂.md`）：边读边交给内核算分叉（[`ForkFeed`]），
//! 整份不同时留在内存里；以前整份读进来、过两遍账本，再把分叉点以前的有效历史攒一遍，只为写一条。
//!
//! 分叉读好的底子交给紧接着的载入（施工 V-39 下，`docs/construction/V-39下-分叉底子交给载入和索引.md`，[`fork_handing`]）：
//! 喂到分叉点的 `BaseFeed`、底子里她看过的文件、回合索引要认的底子（`Priming::base`）跟着分叉这一遍一起喂好，交给
//! 调的一方，它紧接着载入这条支线时交过去，载入、回合索引都不再读父会话。只活在这一次分叉、载入之间，不落盘、不常驻。

use std::fmt;
use std::io;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Forked, Permission, SessionCreated};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{CallId, CommandId, Seq, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{BaseFeed, ForkFeed, ForkPlan, LoadError as Broken, Session};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_policy::{BuildError, Snapshot, SnapshotError};
use miyu_recall::Priming;
use miyu_store::blob::{BlobError, Blobs};
use miyu_store::index::SessionIndex;
use miyu_store::log::{Flow, OpenError, SEGMENT_LIMIT, SessionLog, abandon, read_range};
use miyu_store::root::DataRoot;

use super::scan::Place;
use crate::blocking::blocking;
use crate::effects::SeenScan;
use crate::store::{Indexed, Store};

/// 分叉要的几样。
pub struct Fork<'a> {
    /// 数据根。
    pub root: &'a DataRoot,
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
    /// fork 子代理（施工 A-F 中）：从派它的那次调用那一步分叉。普通分叉没有。
    pub within: Option<Within>,
}

/// fork 子代理的分叉（施工 A-F 中）：派它的那次调用、子会话第几层、占位结果的字。
pub struct Within {
    /// 派它的那次调用：还在等结果的。
    pub call: CallId,
    /// 子会话第几层：父会话的加一。
    pub depth: u32,
    /// 那一步每个调用的占位结果（`core/tool-results/forked.txt`）。
    pub placeholder: String,
    /// 只读（施工 SA-1）：权限照父会话那一刻的抄，再把只读开关打开；父会话只读的本来就开着。
    pub read_only: bool,
}

/// 分叉不成。
#[derive(Debug)]
pub enum ForkError {
    /// 父会话的日志打不开：没有这个会话，或者日志坏了。
    Log(OpenError),
    /// 父会话自己是子会话或者分叉出来的：只能从主会话分叉。fork 子代理（施工 A-F 中）也能从普通子代理分叉，不能从 fork
    /// 子代理、支线分叉。
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

/// 分叉时读好的底子（施工 V-39 下）：父会话喂到分叉点为止的那一份（压缩以前的已经放掉），连同底子里她看过的文件、回合索引
/// 要认的底子里还没回的人话。[`fork_handing`] 交回，交给紧接着载入这条支线的 [`Load::forked`](crate::Load::forked)：
/// 载入、回合索引照它接着走，不再读父会话。
///
/// 只交给那一次载入：不是这条支线、不是这个分叉点的不用（照旧自己读）；没交出去、没用上的跟着放掉，不落盘、不常驻。
pub struct ForkedBase {
    /// 分叉出来的支线。
    pub(crate) child: SessionId,
    /// 父会话。
    pub(crate) parent: SessionId,
    /// 分叉点。
    pub(crate) upto: Seq,
    /// 喂到分叉点的底子。
    pub(crate) base: BaseFeed,
    /// 底子里她看过的文件（同载入时的 `Scan::base`）。
    pub(crate) seen: SeenScan,
    /// 回合索引照底子认的（同回合索引接上时的 `Laying::base`）。
    pub(crate) priming: Priming,
}

impl fmt::Debug for ForkedBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForkedBase")
            .field("child", &self.child)
            .field("parent", &self.parent)
            .field("upto", &self.upto)
            .finish_non_exhaustive()
    }
}

/// 分叉：写好子会话的第一条，交回分叉点。父会话照只读读（它可能正在写）。
///
/// # Errors
///
/// 见 [`ForkError`]。
pub async fn fork(setup: Fork<'_>) -> Result<Seq, ForkError> {
    forked::<()>(setup).await.map(|(upto, ())| upto)
}

/// 同 [`fork`]，另交回这一遍读好的底子（施工 V-39 下，[`ForkedBase`]）：调的一方紧接着载入这条支线时交过去。
///
/// # Errors
///
/// 同 [`fork`]。
pub async fn fork_handing(setup: Fork<'_>) -> Result<(Seq, ForkedBase), ForkError> {
    forked::<Handing>(setup).await
}

/// 分叉，喂底子时另照 `H` 记、写完第一条照它交回（[`Hand`]）。
async fn forked<H: Hand>(setup: Fork<'_>) -> Result<(Seq, H::Handed), ForkError> {
    let Fork {
        root,
        parent,
        id,
        cause,
        by,
        at,
        index,
        seal_after,
        within,
    } = setup;
    let parent_dir = root.session_dir(parent);
    let child_dir = root.session_dir(&id);
    let blobs = Blobs::new(root.blobs());
    let parent = parent.clone();
    blocking(move || {
        let Parent {
            created,
            plan,
            base,
            noted: Noted { place, hand },
        } = read_parent::<H>(
            &parent_dir,
            &parent,
            within.as_ref().map(|within| within.call),
        )?;
        let bytes = blobs.get(&plan.policy).map_err(ForkError::Blob)?;
        let snapshot = Snapshot::from_bytes(&bytes).map_err(ForkError::Snapshot)?;
        let policy = snapshot.policy().map_err(ForkError::Policy)?;
        let (cwd, dirs) = place.remembered();
        let child = SessionCreated {
            venue: created.venue,
            policy: plan.policy,
            // 只读（施工 SA-1）：fork 子代理写了只读的，只读开关打开。
            permission: Permission {
                read_only: plan.permission.read_only
                    || within.as_ref().is_some_and(|within| within.read_only),
                ..plan.permission
            },
            oneshot: false,
            cwd: cwd.clone().or(created.cwd),
            parent: Some(parent.clone()),
            depth: Some(within.as_ref().map_or(1, |within| within.depth)),
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
            // 关着的会话分叉出来的照旧关着（施工 SU-4 中）。
            confined: created.confined,
            forked: Some(Forked {
                from: parent.clone(),
                upto: plan.upto,
                seal_after,
                within: plan.within,
            }),
        };
        // 内核只照底子写第一条，不攒底子的有效历史（施工 V-39 中）：环境照记下的，时区不进第一条。
        let environment = Environment {
            offset: UtcOffset::UTC,
            cwd: cwd.unwrap_or_default(),
            dirs,
        };
        let first_id = id.clone();
        let child_id = id.clone();
        let first = match &within {
            Some(within) => Session::fork_within(
                id,
                &base,
                &within.placeholder,
                cause,
                by,
                at,
                child,
                policy,
                environment,
            ),
            None => Session::fork(id, &base, cause, by, at, child, policy, environment),
        }
        .map_err(ForkError::Kernel)?;
        // 喂好的底子交给紧接着的载入（施工 V-39 下）；不交的到这里就放掉。
        let handed = hand.hand(child_id, parent, plan.upto, base);
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
        Ok((plan.upto, handed))
    })
    .await
}

/// 父会话读完了：它的 `session.created`、分叉要照的几样、喂到分叉点为止的底子、照到分叉点为止的每一条记下的。
struct Parent<H> {
    created: SessionCreated,
    plan: ForkPlan,
    base: BaseFeed,
    noted: Noted<H>,
}

/// 照喂进底子的每一条（到分叉点为止，照先后）记的：工作目录，和交不交底子的那一份（[`Hand`]）。
#[derive(Default)]
struct Noted<H> {
    place: Place,
    hand: H,
}

impl<H: Hand> Noted<H> {
    /// 记一条。
    fn note(&mut self, event: &Event) {
        self.place.note(event);
        self.hand.note(event);
    }
}

/// 分叉读好的底子交不交给载入（施工 V-39 下）：交的（[`Handing`]）喂底子时另记她看过的文件、回合索引认的底子，写完第一条
/// 连同底子交回；不交的（`()`）什么都不记，底子写完第一条就放掉。
trait Hand: Default + Send + 'static {
    /// 交回的。
    type Handed: Send + 'static;

    /// 喂进底子的一条。
    fn note(&mut self, event: &Event);

    /// 写完第一条：支线 `child`、父会话 `parent`、分叉点 `upto`、喂好的底子 `base`。
    fn hand(self, child: SessionId, parent: SessionId, upto: Seq, base: BaseFeed) -> Self::Handed;
}

impl Hand for () {
    type Handed = ();

    fn note(&mut self, _: &Event) {}

    fn hand(self, _: SessionId, _: SessionId, _: Seq, _: BaseFeed) {}
}

/// 交底子的：照喂进底子的每一条记她看过的文件（同载入时的 `Scan::base`）、回合索引认的底子（同接上时的 `Laying::base`）。
#[derive(Default)]
struct Handing {
    seen: SeenScan,
    priming: Priming,
}

impl Hand for Handing {
    type Handed = ForkedBase;

    fn note(&mut self, event: &Event) {
        self.seen.note(event);
        self.priming.base(event);
    }

    fn hand(self, child: SessionId, parent: SessionId, upto: Seq, base: BaseFeed) -> ForkedBase {
        ForkedBase {
            child,
            parent,
            upto,
            base,
            seen: self.seen,
            priming: self.priming,
        }
    }
}

/// 只读地一块块读父会话 `parent` 的日志（在 `dir`，它可能正在写），边读边交给内核算分叉（`call` 是 fork 子代理派它的那次
/// 调用）。报错的先后同以前整份读进来再算：先是日志打不开、坏了（整份读完才知道，所以账本出的错先记着、接着读到底），
/// 再是第一条不是造会话、不能从它分叉，再是账本、分叉点。第一条就不对的不交给内核，照样读完。喂进底子的每一条另照 `H` 记
/// （[`Hand`]）。
fn read_parent<H: Hand>(
    dir: &Path,
    parent: &SessionId,
    call: Option<CallId>,
) -> Result<Parent<H>, ForkError> {
    // 第一块到了没有，到了的第一条是不是造会话那一条。
    let mut created: Option<Option<SessionCreated>> = None;
    let mut feed: Option<ForkFeed> = None;
    let mut noted = Noted::<H>::default();
    read_range(dir, Seq::FIRST, None, |events| {
        if created.is_none() {
            let first = match events.first().map(|event| &event.body) {
                Some(Body::SessionCreated(created)) => Some(created.clone()),
                _ => None,
            };
            if first.as_ref().is_some_and(|first| !refused(first, call)) {
                feed = Some(ForkFeed::new(parent.clone()));
            }
            created = Some(first);
        }
        if let Some(feed) = &mut feed {
            feed.feed(events, &mut |event| noted.note(event));
        }
        Flow::Go
    })
    .map_err(ForkError::Log)?;
    let Some(created) = created.flatten() else {
        return Err(ForkError::Kernel(Broken::Empty));
    };
    let Some(feed) = feed else {
        return Err(ForkError::NotMain);
    };
    let (plan, base) = feed
        .finish(call, &mut |event| noted.note(event))
        .map_err(ForkError::Kernel)?;
    Ok(Parent {
        created,
        plan,
        base,
        noted,
    })
}

/// 不能从这个会话（第一条是 `created`）分叉：子代理、分叉出来的都有父会话。fork 子代理（派它的调用 `call`）只看是不是
/// 分叉出来的：普通子代理也能派它。
fn refused(created: &SessionCreated, call: Option<CallId>) -> bool {
    match call {
        Some(_) => created.forked.is_some(),
        None => created.parent.is_some(),
    }
}

//! 施工 V-39 中以前的 `miyu_session::fork`（origin/main `b8f496958` 的 `open/fork.rs`），照抄当准绳：父会话整份读进来
//! （`read_events`），第一条、能不能分叉，整份交给内核算分叉点，到分叉点的那一截整份交给内核写第一条。只换了三处：内核
//! 以前的 `fork_plan` / `fork_plan_at` 换成整份当一块喂的 [`ForkFeed`]、`Session::fork` 的底子换成整份当一块喂的
//! [`BaseFeed::of`]（这两样和以前的逐字一样由内核的对照测试管，`session/tests/random/forking.rs`）；`remembered` 照抄在
//! 下面；写第一条不更新会话列表（测试不给索引）。

use miyu_kernel::event::{Body, Event, Forked, SessionCreated};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::Seq;
use miyu_kernel::session::{BaseFeed, ForkFeed, LoadError as Broken, Session};
use miyu_kernel::time::UtcOffset;
use miyu_policy::Snapshot;
use miyu_session::{Fork, ForkError};
use miyu_store::blob::Blobs;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog, abandon, read_events};

/// 以前的分叉：写好子会话的第一条，交回分叉点。
pub(super) fn fork(setup: Fork<'_>) -> Result<Seq, ForkError> {
    let Fork {
        root,
        parent,
        id,
        cause,
        by,
        at,
        index: _,
        seal_after,
        within,
    } = setup;
    let parent_dir = root.session_dir(parent);
    let child_dir = root.session_dir(&id);
    let blobs = Blobs::new(root.blobs());
    let parent = parent.clone();
    let events = read_events(&parent_dir).map_err(ForkError::Log)?;
    let created = match events.first().map(|event| &event.body) {
        Some(Body::SessionCreated(created)) => created.clone(),
        _ => return Err(ForkError::Kernel(Broken::Empty)),
    };
    let refused = match within {
        Some(_) => created.forked.is_some(),
        None => created.parent.is_some(),
    };
    if refused {
        return Err(ForkError::NotMain);
    }
    let mut feed = ForkFeed::new(parent.clone());
    feed.feed(events.clone(), &mut |_| {});
    let plan = feed
        .finish(within.as_ref().map(|within| within.call), &mut |_| {})
        .map(|(plan, _)| plan)
        .map_err(ForkError::Kernel)?;
    let base: Vec<Event> = events
        .into_iter()
        .take_while(|event| event.seq <= plan.upto)
        .collect();
    let bytes = blobs.get(&plan.policy).map_err(ForkError::Blob)?;
    let snapshot = Snapshot::from_bytes(&bytes).map_err(ForkError::Snapshot)?;
    let policy = snapshot.policy().map_err(ForkError::Policy)?;
    let (cwd, dirs) = remembered(&base);
    let child = SessionCreated {
        venue: created.venue,
        policy: plan.policy,
        permission: plan.permission,
        oneshot: false,
        cwd: cwd.clone().or(created.cwd),
        parent: Some(parent.clone()),
        depth: Some(within.as_ref().map_or(1, |within| within.depth)),
        workspace_from: created.workspace_from.or_else(|| Some(parent.clone())),
        model: plan.model,
        persona: plan.persona,
        preset: plan.preset.or(created.preset),
        no_tools: created.no_tools,
        images: created.images,
        titles: created.titles,
        dream: None,
        origin: None,
        confined: created.confined,
        forked: Some(Forked {
            from: parent.clone(),
            upto: plan.upto,
            seal_after,
            within: plan.within,
        }),
    };
    let environment = Environment {
        offset: UtcOffset::UTC,
        cwd: cwd.unwrap_or_default(),
        dirs,
    };
    let fed = BaseFeed::of(&parent, &base);
    let first = match &within {
        Some(within) => Session::fork_within(
            id,
            &fed,
            &within.placeholder,
            cause,
            by,
            at,
            child,
            policy,
            environment,
        ),
        None => Session::fork(id, &fed, cause, by, at, child, policy, environment),
    }
    .map_err(ForkError::Kernel)?;
    let start = plan.upto.next();
    let mut log =
        SessionLog::create_at(&child_dir, SEGMENT_LIMIT, start).map_err(ForkError::Disk)?;
    if let Err(error) = log.append(&first) {
        drop(log);
        drop(abandon(&child_dir));
        return Err(ForkError::Disk(error));
    }
    Ok(plan.upto)
}

/// 以前的 `remembered`：日志里最后一次记下的工作目录、加进来的目录。
fn remembered(events: &[Event]) -> (Option<String>, Vec<String>) {
    let cwd = events
        .iter()
        .rev()
        .find_map(|event| miyu_store::index::cwd(event).map(str::to_string));
    let dirs = events
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::TurnStarted(started) => Some(started.dirs.clone()),
            Body::WorkspaceChanged(changed) => changed.dirs.clone(),
            _ => None,
        })
        .unwrap_or_default();
    (cwd, dirs)
}

//! 补齐旧会话（施工 R-2 下，`docs/blueprint/memory.md`「怎么走」第一条第 9 款）：人格那一间的回合库是新建的、重建过的，在
//! 后台照数据根里的会话一个个补。哪些会话照载入时的判法（最近一次换上的快照：主会话、范围是 `persona`、人格是这一间的），
//! 怎么补照载入时那一套（[`Turns::connect`]：照到的位置以后的补，一条都没照过的整份补）。在跑的会话照样补：补的一批落后的
//! 检索库不写（`recall.md`），不会把会话自己撤销了的又放回去。

use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_policy::Snapshot;
use miyu_policy::memory::MemoryScope;
use miyu_store::blob::Blobs;
use miyu_store::log::read_events;
use miyu_store::recall::{NO_PERSONA, Origin, RecallIndexes, Room};
use miyu_store::root::DataRoot;

use crate::TARGET;
use crate::open::current_policy;

use super::{Turns, scope, subagent};

/// 在跑的补齐（施工 R-28）：[`Memory::settle`](super::Memory::settle) 等它们补完。
#[derive(Debug, Default)]
pub(crate) struct Backfills(Mutex<Vec<JoinHandle<()>>>);

impl Backfills {
    /// 起一个后台线程补 `room`（运行日志里的短号是 `room_id`，施工 LG-1 下）；起不来的记一行。
    pub(crate) fn spawn(
        &self,
        recall: Arc<RecallIndexes>,
        root: DataRoot,
        room: Room,
        room_id: String,
    ) {
        let spawned = std::thread::Builder::new()
            .name("memory-backfill".to_string())
            .spawn(move || backfill(&recall, &root, &room, &room_id));
        match spawned {
            Ok(handle) => self.lock().push(handle),
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "memory index not backfilled");
            }
        }
    }

    /// 起一个名叫 `name` 的后台线程做 `work`（施工 SE-1 下：导进来的会话），[`Backfills::settle`] 照样等它；起不来的记一行。
    pub(crate) fn run(&self, name: &str, work: impl FnOnce() + Send + 'static) {
        match std::thread::Builder::new()
            .name(name.to_string())
            .spawn(work)
        {
            Ok(handle) => self.lock().push(handle),
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "imported sessions not indexed");
            }
        }
    }

    /// 等这时在跑的都补完；补的线程 panic 了的记一行。
    pub(crate) fn settle(&self) {
        let running = std::mem::take(&mut *self.lock());
        for handle in running {
            if handle.join().is_err() {
                tracing::error!(target: TARGET, "memory backfill panicked");
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<JoinHandle<()>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 补房间 `room`（人格那一间）：一个个会话读日志、照判法补，读不了的记一行跳过，补完记一行补了几个。运行日志里
/// 这一间写种类和短号 `room_id`（施工 LG-1 下）。
pub(crate) fn backfill(recall: &Arc<RecallIndexes>, root: &DataRoot, room: &Room, room_id: &str) {
    let Room::Persona { persona, .. } = room;
    let room_kind = room.kind();
    let sessions = match root.sessions() {
        Ok(sessions) => sessions,
        Err(error) => {
            tracing::warn!(target: TARGET, room_kind, room_id, error = %error, "memory index not backfilled");
            return;
        }
    };
    let blobs = Blobs::new(root.blobs());
    let mut filled = 0;
    for session in sessions {
        match one(recall, root, &blobs, Some((room, persona)), &session) {
            Ok(true) => filled += 1,
            Ok(false) => {}
            Err(why) => {
                tracing::warn!(target: TARGET, session = %session, error = why.as_str(), "memory index not backfilled");
            }
        }
    }
    tracing::info!(target: TARGET, room_kind, room_id, sessions = filled, "memory index backfilled");
}

/// 导进来的会话（施工 SE-1 下，设计 58 第二节第 8 条）：一个个照它自己快照里的人格，接进那一间的回合库，不载入、不抽取。
/// 读不了的记一行跳过；补完记一行补了几个。人格那一间照快照里的编号：迁移程序写日志时就写了这里的人格（不对映）。
pub(crate) fn imported(recall: &Arc<RecallIndexes>, root: &DataRoot, sessions: &[SessionId]) {
    let blobs = Blobs::new(root.blobs());
    let mut filled = 0;
    for session in sessions {
        match one(recall, root, &blobs, None, session) {
            Ok(true) => filled += 1,
            Ok(false) => {}
            Err(why) => {
                tracing::warn!(target: TARGET, session = %session, error = why.as_str(), "imported session not indexed");
            }
        }
    }
    tracing::info!(target: TARGET, sessions = filled, "imported sessions indexed");
}

/// 补一个会话：照载入时的判法该进这一间的，照载入时那一套补，交回 `true`；不该进的 `false`。`room` 是补哪一间（人格那一间
/// 和它的人格）；不写的（导进来的会话，施工 SE-1 下）照它快照里的人格进那一间，日志只读一遍。
fn one(
    recall: &Arc<RecallIndexes>,
    root: &DataRoot,
    blobs: &Blobs,
    room: Option<(&Room, &str)>,
    session: &SessionId,
) -> Result<bool, String> {
    let events = read_events(&root.session_dir(session)).map_err(|error| error.to_string())?;
    let Some(Event {
        body: Body::SessionCreated(created),
        ..
    }) = events.first()
    else {
        return Err("the first event is not session.created".to_string());
    };
    let hash = current_policy(&events).unwrap_or(&created.policy);
    let bytes = blobs.get(hash).map_err(|error| error.to_string())?;
    let snapshot = Snapshot::from_bytes(&bytes).map_err(|error| error.to_string())?;
    let scope = scope(subagent(created), true, snapshot.memory_scope());
    let own = snapshot.persona.as_deref().unwrap_or(NO_PERSONA);
    let own_room;
    let (room, persona) = match room {
        Some(given) => given,
        None => {
            own_room = Room::persona(own);
            (&own_room, own)
        }
    };
    // 场所会话的回合也补（施工 R-13 下，同载入时，`memory.rs` 的 `connect`）：回合库记下它在哪个场所。
    if scope != MemoryScope::Persona || own != persona {
        return Ok(false);
    }
    // 分叉出来的支线照载入时一样读底子（施工 R-13 三补）：父会话到分叉点为止的，认得那里还没回的人话。
    let base = match &created.forked {
        Some(forked) => read_events(&root.session_dir(&forked.from))
            .map_err(|error| error.to_string())?
            .into_iter()
            .take_while(|event| event.seq <= forked.upto)
            .collect(),
        None => Vec::new(),
    };
    // 中途换成这个人格的（施工 P-13）：换过来以前的几轮不在这一间记。
    let floor = super::recast::floor(&events, persona);
    let (base, own) = super::recast::split(&base, &events, floor);
    let origin = Origin {
        main: created.parent.is_none(),
    };
    Turns::connect(recall, room, (session, origin), &created.venue, &base, own);
    Ok(true)
}

//! 回合索引在后台写（施工 V-9，`docs/blueprint/perf.md`「怎么走」第 5 条）：接上（开回合库、对照到哪、照整份事件铺回
//! `TurnFeed`）和每落一批以后的写库都交给核心一个后台线程，载入、追加不等它。开回合库要跑一遍 `quick_check`，跟着人格的
//! 那一间攒着这个人格所有会话的回合，越用越大，原来整笔压在打开会话上。
//!
//! 一个核心一个线程，照交来的先后做：同一个会话的接上、几批照先后，一批不会赶在接上前面。接上时自己读日志，已经铺过的那几条
//! 后来又交来的不再看。回合库本来就照「照到哪」认落后的一批不写（`RecallIndex::apply`），会话停了、核心退了还没写的，下次
//! 载入照日志补上。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, mpsc};

use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId, VenueId};
use miyu_store::log::{SessionLog, read_events};
use miyu_store::recall::{RecallIndexes, Room};

use super::Turns;
use crate::TARGET;

/// 核心的那个后台线程：头一回用到时起，交活的端口一直留着。
#[derive(Default)]
pub(crate) struct Lanes {
    jobs: OnceLock<Option<mpsc::Sender<Job>>>,
    next: AtomicU64,
    /// 交了还没做完的几件：没有的等它不用跑一趟后台。
    pending: Arc<AtomicUsize>,
}

impl std::fmt::Debug for Lanes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lanes")
            .field("started", &self.jobs.get().is_some())
            .finish_non_exhaustive()
    }
}

/// 一个会话交给后台线程的那一条道：放下时后台照样把交过的写完，再放下这个会话的回合索引。
pub(crate) struct TurnLane {
    id: u64,
    jobs: mpsc::Sender<Job>,
    pending: Arc<AtomicUsize>,
}

/// 接上要的几样：回合库在哪一间、会话、场所，抽取从哪一条以后算（中途换过人格的，施工 P-13），一开始铺什么、日志在哪。
pub(crate) struct Opening {
    pub(crate) recall: Arc<RecallIndexes>,
    pub(crate) room: Room,
    pub(crate) session: SessionId,
    pub(crate) venue: VenueId,
    pub(crate) floor: Option<Seq>,
    pub(crate) laid: Laid<'static>,
    pub(crate) source: Source,
}

/// 一开始照会话自己的哪些事件铺。
pub(crate) enum Laid<'a> {
    /// 新造的会话：什么都没有，不开库。
    Empty,
    /// 载入时读到的：这里只拿来认换人格的那一条，后台自己再读一遍日志铺，不占载入。
    Read(&'a [Event]),
    /// 交来的整份（换人格时已经读回的）。
    Given(Vec<Event>),
}

impl<'a> Laid<'a> {
    /// 会话自己的事件：后台再读的这时就有的那一份。
    pub(crate) fn own(&self) -> &[Event] {
        match self {
            Laid::Empty => &[],
            Laid::Read(events) => events,
            Laid::Given(events) => events,
        }
    }

    /// 交给后台的：载入时读到的不带过去。
    pub(crate) fn sent(self) -> Laid<'static> {
        match self {
            Laid::Empty => Laid::Empty,
            Laid::Read(_) => Laid::Read(&[]),
            Laid::Given(events) => Laid::Given(events),
        }
    }
}

/// 会话的日志：目录，和分叉出来的支线的底子（施工 C-8 上）。读回时底子接在前面。
pub(crate) struct Source {
    pub(crate) dir: PathBuf,
    pub(crate) base: Option<Arc<Vec<Event>>>,
}

enum Job {
    Open(u64, Box<Opening>),
    Advance(u64, Vec<Event>),
    Close(u64),
    /// 交过的都做完了就回一声。
    Settle(mpsc::Sender<()>),
}

/// 接上了的一个会话：回合索引，铺到了哪一条，日志在哪。
struct Open {
    turns: Turns,
    upto: Option<Seq>,
    source: Source,
}

impl Lanes {
    /// 交上一个会话：后台照 `opening` 接上，交回往后每落一批用的那一条道。线程起不来的没有（记一行，下次载入照日志补）。
    pub(crate) fn open(&self, opening: Opening) -> Option<TurnLane> {
        let pending = Arc::clone(&self.pending);
        let jobs = self
            .jobs
            .get_or_init(|| start(Arc::clone(&pending)))
            .clone()?;
        let lane = TurnLane {
            id: self.next.fetch_add(1, Ordering::Relaxed),
            jobs,
            pending,
        };
        lane.send(Job::Open(lane.id, Box::new(opening)));
        Some(lane)
    }

    /// 等后台把这时交过的都做完：搜回合库、删会话以前（读到的是落了盘的全部）。没有没做完的不等。
    pub(crate) fn settle(&self) {
        let Some(Some(jobs)) = self.jobs.get() else {
            return;
        };
        if self.pending.load(Ordering::Acquire) == 0 {
            return;
        }
        let (done, wait) = mpsc::channel();
        if jobs.send(Job::Settle(done)).is_err() || wait.recv().is_err() {
            tracing::warn!(target: TARGET, "memory index not updated");
        }
    }
}

impl TurnLane {
    /// 一批事件落了盘：交给后台。
    pub(crate) fn advance(&self, events: &[Event]) {
        if !events.is_empty() {
            self.send(Job::Advance(self.id, events.to_vec()));
        }
    }

    /// 交一件，记一件没做完的（后台不在了的不记）。
    fn send(&self, job: Job) {
        self.pending.fetch_add(1, Ordering::AcqRel);
        if self.jobs.send(job).is_err() {
            self.pending.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl Drop for TurnLane {
    fn drop(&mut self) {
        self.send(Job::Close(self.id));
    }
}

fn start(pending: Arc<AtomicUsize>) -> Option<mpsc::Sender<Job>> {
    let (jobs, inbox) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("memory-turns".to_string())
        .spawn(move || run(&inbox, &pending));
    match spawned {
        Ok(_) => Some(jobs),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "memory index not updated");
            None
        }
    }
}

fn run(inbox: &mpsc::Receiver<Job>, pending: &AtomicUsize) {
    let mut open: HashMap<u64, Open> = HashMap::new();
    for job in inbox {
        match job {
            Job::Open(id, opening) => {
                if let Some(opened) = connect(*opening) {
                    open.insert(id, opened);
                }
            }
            Job::Advance(id, events) => {
                if let Some(opened) = open.get_mut(&id) {
                    opened.advance(events);
                }
            }
            Job::Close(id) => drop(open.remove(&id)),
            Job::Settle(done) => {
                // 等的那一方不等了的不要紧。
                done.send(()).unwrap_or_default();
                continue;
            }
        }
        pending.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 照 `opening` 接上：读不了日志的这一回不记（下次载入照日志补）。
fn connect(opening: Opening) -> Option<Open> {
    let Opening {
        recall,
        room,
        session,
        venue,
        floor,
        laid,
        source,
    } = opening;
    let own = match laid {
        Laid::Empty => Vec::new(),
        Laid::Given(own) => own,
        Laid::Read(_) => match read_events(&source.dir) {
            Ok(own) => own,
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "memory index not updated");
                return None;
            }
        },
    };
    let base = source.base.as_deref().map_or(&[][..], Vec::as_slice);
    let (base, mine) = super::recast::split(base, &own, floor);
    let turns = Turns::connect(&recall, &room, &session, &venue, &base, mine)?;
    let upto = own.last().map(|event| event.seq);
    Some(Open {
        turns,
        upto,
        source,
    })
}

impl Open {
    /// 一批：接上时已经铺过的那几条不再看。
    fn advance(&mut self, mut events: Vec<Event>) {
        if let Some(upto) = self.upto {
            events.retain(|event| event.seq > upto);
        }
        let Some(last) = events.last() else {
            return;
        };
        self.upto = Some(last.seq);
        let source = &self.source;
        self.turns.advance(&events, || source.events());
    }
}

impl Source {
    /// 会话日志 `log` 的，分叉出来的支线带上底子 `base`。
    pub(crate) fn of(log: &SessionLog, base: Option<Arc<Vec<Event>>>) -> Source {
        Source {
            dir: log.dir().to_path_buf(),
            base,
        }
    }

    /// 底子接上会话自己的整份日志。
    fn events(&self) -> Result<Vec<Event>, String> {
        let mut events = self.base.as_deref().cloned().unwrap_or_default();
        events.extend(read_events(&self.dir).map_err(|error| error.to_string())?);
        Ok(events)
    }
}

#[cfg(test)]
mod tests;

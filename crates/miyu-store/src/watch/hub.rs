//! 整个进程共用的一组监视线程（施工 LW-11，`internal/blueprint/config.md`「怎么走」第七条第 6 条）：一个系统的监视器
//! （`notify` 自己的一条线程）看所有订阅的目录，一条 `miyu-watch` 线程收事件、照每一份订阅各自的表认、各自去抖。以前每一份
//! 订阅各开这两条，常驻的核心监视六份就是十二条。
//!
//! 1. 第一次有人订时才开 `miyu-watch`，之后一直在（核心是常驻的，简单为上）；共用的系统监视器第一次要用时才造，造不起来的
//!    这一份退回轮询，下一份再试。
//! 2. 一份订阅的目录和共用的那一个已经在看的一样，或者在它下面、上面又有一边是递归的，这一份自己开一个系统的监视器：
//!    `notify` 一个监视器里同一个目录只记一次，两份订阅叠在一起，一份退订会把另一份要看的一起退掉；被删了重建的目录，
//!    新来的订阅要看的是新的那一个。自己开的和以前一样，事件也送进 `miyu-watch`，只交给这一份。
//! 3. 系统的监视加一个目录时会停下别的目录的（macOS 的 FSEvents 要把整条流停下重开，中间的变动收不到），每一份也自己开：
//!    共用的会让别的订阅漏掉变动。
//! 4. 交出去不在 `miyu-watch` 上：去抖到点的文件排进这一份自己的队，开一条短命的 `miyu-watch-call` 线程挨个交，交完就退。
//!    一份订阅同一时刻只有一个在交（和以前它自己那一条线程一样，一个接一个）；交得慢、卡住的，只耽误它自己，别的照样交。
//!    交的时候 panic 了，这一份停下、不再交（以前是它自己的线程崩了），别的不受影响。
//! 5. 共用的监视器报错、事件太多丢了的，所有用着它的订阅都当它们的几份都变了：它们共用一份系统的事件队列，丢都一起丢。

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use notify::{Event, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher, WatcherKind};

use super::{Backend, POLL, Table, Watch};

/// 调用的一方给的、交出去时调的。
pub(super) type OnChange = Box<dyn Fn(&Path) + Send>;

/// 整个进程的那一个。还没有人订过的是空的。
static HUB: Mutex<Option<Hub>> = Mutex::new(None);

/// 监视中心：往 `miyu-watch` 送登记、事件的那一头，共用的系统监视器和它在看的目录。
struct Hub {
    send: Sender<Message>,
    /// 共用的系统监视器；还没造过、上一次造不起来的是空的。
    native: Option<RecommendedWatcher>,
    /// 共用的那一个在看的目录（真实的位置）和递归不递归。
    held: Vec<(PathBuf, bool)>,
    /// 下一份订阅的号。
    next: u64,
}

/// 送进 `miyu-watch` 的。登记、退订和事件走同一条管道：登记排在这一份的目录开始看之前，退订排在之后的事件前头。
pub(super) enum Message {
    /// 一份新的订阅。
    Add(u64, Sub),
    /// 这一份不在共用的那一个上（试过、起不来，退回了轮询）：共用的事件不再交给它。
    Alone(u64),
    /// 这一份退订了。
    Remove(u64),
    /// 系统的监视报来的，从哪一个监视器来。
    Event(Source, notify::Result<Event>),
}

/// 事件从哪一个监视器来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    /// 共用的那一个：交给每一份用着它的订阅，各自照表认。
    Shared,
    /// 这一份订阅自己开的（第 2、3 条、退回的轮询）：只交给它。
    Own(u64),
}

/// `miyu-watch` 手里的一份订阅：表、去抖多久、还没到点的、交给谁。
pub(super) struct Sub {
    table: Table,
    quiet: Duration,
    /// 用不用共用的那一个的事件。
    shared: bool,
    /// 碰到的每一份最后一次动的时刻。
    pending: Vec<(PathBuf, Instant)>,
    lane: Arc<Lane>,
}

impl Sub {
    /// 照表 `table` 认、`quiet` 里没有新的变动了交给 `on_change`；`shared` 是用不用共用的那一个的事件。
    pub(super) fn new(table: Table, quiet: Duration, shared: bool, on_change: OnChange) -> Sub {
        Sub {
            table,
            quiet,
            shared,
            pending: Vec::new(),
            lane: Arc::new(Lane {
                on_change: Mutex::new(on_change),
                queue: Mutex::default(),
            }),
        }
    }

    /// 听不听 `source` 来的事件（这一份的号是 `id`）。
    fn hears(&self, id: u64, source: Source) -> bool {
        match source {
            Source::Shared => self.shared,
            Source::Own(own) => own == id,
        }
    }
}

/// 一份订阅交出去的那一条道：到点的文件排队，同一时刻只有一条线程在交。
struct Lane {
    /// 调用的一方给的。交的时候 panic 了，锁就坏了：这一份停下（第 4 条）。
    on_change: Mutex<OnChange>,
    queue: Mutex<Queue>,
}

/// 排着的文件，和有没有线程在交。
#[derive(Default)]
struct Queue {
    files: VecDeque<PathBuf>,
    running: bool,
}

impl Lane {
    /// 排上 `file`（排着的不重排）；没有线程在交的，开一条。
    fn hand(self: &Arc<Self>, file: PathBuf) {
        let mut queue = lock(&self.queue);
        if !queue.files.contains(&file) {
            queue.files.push_back(file);
        }
        if std::mem::replace(&mut queue.running, true) {
            return;
        }
        drop(queue);
        let lane = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("miyu-watch-call".to_string())
            .spawn(move || lane.drain());
        if spawned.is_err() {
            // 线程开不了（进程的线程数到顶这类）：在 `miyu-watch` 上交，慢的会耽误别的订阅，但这一次不丢。
            self.drain();
        }
    }

    /// 挨个交排着的，交完就回。
    fn drain(&self) {
        loop {
            let file = {
                let mut queue = lock(&self.queue);
                let Some(file) = queue.files.pop_front() else {
                    queue.running = false;
                    return;
                };
                file
            };
            let Ok(on_change) = self.on_change.lock() else {
                // 上一回交的时候 panic 了：这一份停下，排着的扔掉（`miyu-watch` 看到坏了的锁就把它摘掉）。
                let mut queue = lock(&self.queue);
                queue.files.clear();
                queue.running = false;
                return;
            };
            (*on_change)(&file);
        }
    }

    /// 交的时候 panic 过：停下了。
    fn stopped(&self) -> bool {
        self.on_change.is_poisoned()
    }
}

/// 拿锁；别的线程拿着锁时 panic 了的照样用：里面的只是排着的文件，没有改到一半的。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 照排好的表 `table` 开始监视（`super::begin`）：登记进 `miyu-watch`，能用共用的那一个的用它，不能的自己开一个。
///
/// # Errors
///
/// `miyu-watch` 开不了；自己开的轮询也起不来（目录没有、线程开不了）：原因。
pub(super) fn join(
    table: Table,
    backend: Backend,
    quiet: Duration,
    on_change: OnChange,
) -> Result<Watch, String> {
    let mut guard = lock(&HUB);
    let hub = match &mut *guard {
        Some(hub) => hub,
        None => guard.insert(Hub::start()?),
    };
    hub.join(table, backend, quiet, on_change)
}

/// 退订 `id`（[`Watch`] 丢掉时）：交不再给它；它在共用的那一个上看着的目录 `shared` 不再看。
pub(super) fn leave(id: u64, shared: &[(PathBuf, bool)]) {
    let mut guard = lock(&HUB);
    let Some(hub) = guard.as_mut() else {
        return;
    };
    hub.tell(Message::Remove(id));
    for dir in shared {
        if let Some(at) = hub.held.iter().position(|held| held == dir) {
            hub.held.swap_remove(at);
        }
        if let Some(native) = &mut hub.native {
            unwatch(native, &dir.0);
        }
    }
}

impl Hub {
    /// 开 `miyu-watch`。
    fn start() -> Result<Hub, String> {
        let (send, messages) = channel();
        std::thread::Builder::new()
            .name("miyu-watch".to_string())
            .spawn(move || dispatch(&messages))
            .map_err(|error| error.to_string())?;
        Ok(Hub {
            send,
            native: None,
            held: Vec::new(),
            next: 0,
        })
    }

    /// 送一条给 `miyu-watch`。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "收的那一头只在 `miyu-watch` 退了时才没有；它只在送的这一头都没了才退，而这一头一直在 `HUB` 里"
    )]
    fn tell(&self, message: Message) {
        let _ = self.send.send(message);
    }

    /// 登记一份，开始看。
    fn join(
        &mut self,
        table: Table,
        backend: Backend,
        quiet: Duration,
        on_change: OnChange,
    ) -> Result<Watch, String> {
        let id = self.next;
        self.next += 1;
        let dirs = table.dirs();
        let share = matches!(backend, Backend::Native) && shareable() && !self.overlaps(&dirs);
        // 先登记，再看：看上以后的事件都排在登记后面。
        self.tell(Message::Add(id, Sub::new(table, quiet, share, on_change)));
        let started = match share {
            true => match self.share(&dirs) {
                Ok(()) => Ok((None, dirs, None)),
                Err(error) => {
                    self.tell(Message::Alone(id));
                    own(&dirs, Backend::Poll(POLL), &self.send, id)
                        .map(|(watcher, _)| (Some(watcher), Vec::new(), Some(error.to_string())))
                        .map_err(|poll| format!("{error}; polling: {poll}"))
                }
            },
            false => own(&dirs, backend, &self.send, id)
                .map(|(watcher, unavailable)| (Some(watcher), Vec::new(), unavailable)),
        };
        match started {
            Ok((own, shared, unavailable)) => Ok(Watch {
                id,
                _own: own,
                shared,
                unavailable,
            }),
            Err(error) => {
                self.tell(Message::Remove(id));
                Err(error)
            }
        }
    }

    /// `dirs` 里有没有和共用的那一个已经在看的叠在一起的（第 2 条）：一样的；在它下面、上面，又有一边是递归的。`dirs`
    /// 自己里面叠着的也算。
    fn overlaps(&self, dirs: &[(PathBuf, bool)]) -> bool {
        dirs.iter().enumerate().any(|(at, (dir, tree))| {
            self.held
                .iter()
                .chain(&dirs[..at])
                .any(|(held, held_tree)| {
                    dir == held
                        || (*held_tree && dir.starts_with(held))
                        || (*tree && held.starts_with(dir))
                })
        })
    }

    /// 让共用的那一个也看 `dirs`（还没造的先造）。看不了的，这一次加上的都退掉，交回原因。
    fn share(&mut self, dirs: &[(PathBuf, bool)]) -> notify::Result<()> {
        let native = match &mut self.native {
            Some(native) => native,
            None => self.native.insert(RecommendedWatcher::new(
                handler(&self.send, Source::Shared),
                notify::Config::default(),
            )?),
        };
        for (at, (dir, tree)) in dirs.iter().enumerate() {
            if let Err(error) = native.watch(dir, mode(*tree)) {
                for (dir, _) in &dirs[..at] {
                    unwatch(native, dir);
                }
                return Err(error);
            }
        }
        self.held.extend(dirs.iter().cloned());
        Ok(())
    }
}

/// 系统的监视加一个目录时，别的目录照样看着、不漏（第 3 条）。问的是监视器是哪一种：只有 FSEvents 要停下重开。
fn shareable() -> bool {
    !matches!(RecommendedWatcher::kind(), WatcherKind::Fsevent)
}

/// 共用的那一个不再看 `dir`。
#[expect(
    clippy::let_underscore_must_use,
    reason = "退订时没有谁能接这个错：看不了了多半是目录被删了，系统已经不看它；以前丢掉自己那个监视器也是这样不报"
)]
fn unwatch(native: &mut RecommendedWatcher, dir: &Path) {
    let _ = native.unwatch(dir);
}

/// 一份订阅自己开的监视器（第 2、3 条，或者只轮询）：照 `backend` 看 `dirs`，系统的起不来的换成每 [`POLL`] 轮询，交回起不来的
/// 原因；事件送进 `miyu-watch`，只交给 `id`。
fn own(
    dirs: &[(PathBuf, bool)],
    backend: Backend,
    send: &Sender<Message>,
    id: u64,
) -> Result<(Box<dyn Watcher + Send>, Option<String>), String> {
    match backend {
        Backend::Poll(every) => {
            let config = notify::Config::default().with_poll_interval(every);
            let watcher = PollWatcher::new(handler(send, Source::Own(id)), config)
                .and_then(|watcher| watch_all(watcher, dirs))
                .map_err(|error| error.to_string())?;
            Ok((Box::new(watcher), None))
        }
        Backend::Native => {
            match RecommendedWatcher::new(handler(send, Source::Own(id)), notify::Config::default())
                .and_then(|watcher| watch_all(watcher, dirs))
            {
                Ok(watcher) => Ok((Box::new(watcher), None)),
                Err(error) => {
                    let (watcher, _) = own(dirs, Backend::Poll(POLL), send, id)
                        .map_err(|poll| format!("{error}; polling: {poll}"))?;
                    Ok((watcher, Some(error.to_string())))
                }
            }
        }
    }
}

/// 让 `watcher` 看 `dirs` 里的每一个。
fn watch_all<W: Watcher>(mut watcher: W, dirs: &[(PathBuf, bool)]) -> notify::Result<W> {
    for (dir, tree) in dirs {
        watcher.watch(dir, mode(*tree))?;
    }
    Ok(watcher)
}

/// 递归不递归。
fn mode(tree: bool) -> RecursiveMode {
    match tree {
        true => RecursiveMode::Recursive,
        false => RecursiveMode::NonRecursive,
    }
}

/// 监视器的事件送进 `miyu-watch`，记着从哪一个来。
#[expect(
    clippy::let_underscore_must_use,
    reason = "`miyu-watch` 一直在（`Hub::tell`），送不出去的只可能是进程在退"
)]
fn handler(
    send: &Sender<Message>,
    source: Source,
) -> impl FnMut(notify::Result<Event>) + Send + 'static {
    let send = send.clone();
    move |event| {
        let _ = send.send(Message::Event(source, event));
    }
}

/// `miyu-watch`：收登记、退订和事件，每一份订阅记下碰到的每一份最后一次动的时刻，过了它的 `quiet` 没有新的变动了交出去。送的
/// 那一头都没了就退（只在测试里：`HUB` 一直拿着一头）。
pub(super) fn dispatch(messages: &Receiver<Message>) {
    let mut subs: HashMap<u64, Sub> = HashMap::new();
    loop {
        let due = subs
            .values()
            .flat_map(|sub| sub.pending.iter().map(|(_, at)| *at + sub.quiet))
            .min();
        let received = match due {
            Some(due) => messages.recv_timeout(due.saturating_duration_since(Instant::now())),
            None => messages.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let now = Instant::now();
        match received {
            Ok(Message::Add(id, sub)) => {
                subs.insert(id, sub);
            }
            Ok(Message::Alone(id)) => {
                if let Some(sub) = subs.get_mut(&id) {
                    sub.shared = false;
                }
            }
            Ok(Message::Remove(id)) => {
                subs.remove(&id);
            }
            Ok(Message::Event(source, event)) => {
                for (id, sub) in &mut subs {
                    if !sub.hears(*id, source) {
                        continue;
                    }
                    let touched = match &event {
                        Ok(event) => sub.table.touched(event),
                        // 监视报错：当这几份都变了，重读一遍（`super` 第 6 条）。
                        Err(_) => sub.table.files(),
                    };
                    for file in touched {
                        sub.pending.retain(|(waiting, _)| *waiting != file);
                        sub.pending.push((file, now));
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        subs.retain(|_, sub| !sub.lane.stopped());
        for sub in subs.values_mut() {
            let quiet = sub.quiet;
            let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut sub.pending)
                .into_iter()
                .partition(|(_, at)| now.saturating_duration_since(*at) >= quiet);
            sub.pending = waiting;
            for (file, _) in ready {
                sub.lane.hand(file);
            }
        }
    }
}

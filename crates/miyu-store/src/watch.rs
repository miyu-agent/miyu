//! 监视几份文件（`docs/blueprint/config.md`「怎么走」第七条，施工 8-4）：配置文件、信任的记录手改了，核心当场知道。
//!
//! 1. 看的是文件所在的目录，不是文件本身，不递归：很多编辑器存盘是先写新文件再改名，看文件会跟丢（`14-配置.md` 第五节）。
//!    文件是链接的，另外看它本体所在的目录。
//! 2. 路径都先换成真实的位置再比：macOS 的 FSEvents 报的是真实的路径，`/var` 这类是链接，照原样比会认不出来。
//! 3. 目录里有变动，只认要看的那几个文件名，别的（临时文件、编辑器的备份）不理。只读的动静（打开、读完关上）也不理：
//!    核心重读一遍也是这样的动静，认了就会一直重读下去。
//! 4. 一份文件 200 毫秒里没有新的变动了，才交出去：连着来的几下（写、改名、改属性）合成一次。
//! 5. 系统的监视起不来（Linux 上 inotify 的名额用完了这类），退回每 2 秒看一次修改时间和长短（`notify` 的轮询），交回
//!    起不来的原因，由调用的一方记日志。
//! 6. 监视报错、事件太多丢了的，当这几份都变了：重读一遍，一样的调用的一方什么都不做。
//!
//! 交出去的是调用的一方给的那个路径（不是真实的位置），在监视自己的线程上调 `on_change`。丢掉 [`Watch`] 就停。
//!
//! 也能看一整个目录（[`watch_dir`]，施工 X-12 补，家目录里的脚本）：目录里点开头的以外，哪个名字变了都算，交出去的是这个目录；
//! 不递归，别的同上。

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use notify::event::{AccessKind, AccessMode};
use notify::{Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};

/// 一份文件这么久没有新的变动了，才交出去（第七条第 3 条）。
pub const QUIET: Duration = Duration::from_millis(200);

/// 系统的监视起不来时，多久看一次（第七条第 4 条）。
pub const POLL: Duration = Duration::from_secs(2);

/// 在监视着：丢掉它就停，监视的线程跟着退。
pub struct Watch {
    /// 系统的监视或者轮询：它拿着送事件的那一头，丢掉它，监视的线程收不到了就退。
    _watcher: Box<dyn Watcher + Send>,
    /// 系统的监视起不来的原因：退回了轮询。
    unavailable: Option<String>,
}

impl Watch {
    /// 系统的监视起不来、退回了轮询的，为什么起不来。
    pub fn unavailable(&self) -> Option<&str> {
        self.unavailable.as_deref()
    }
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watch")
            .field("unavailable", &self.unavailable)
            .finish_non_exhaustive()
    }
}

/// 监视 `files` 这几份：哪一份变了（合并过的），在监视的线程上调 `on_change`，交的是 `files` 里的那个路径。先用系统的
/// 监视，起不来的退回轮询（[`Watch::unavailable`] 说为什么）。文件所在的目录要已经有了。
///
/// # Errors
///
/// 轮询也起不来（目录没有、线程开不了）：原因。
pub fn watch(
    files: &[PathBuf],
    on_change: impl Fn(&Path) + Send + 'static,
) -> Result<Watch, String> {
    start(files, Backend::Native, QUIET, on_change)
}

/// 监视目录 `dir` 里的文件（施工 X-12 补）：点开头的以外哪一个变了（合并过的），在监视的线程上调 `on_change`，交的是 `dir`。
/// 子文件夹里的不算。目录要已经有了。别的同 [`watch`]。
///
/// # Errors
///
/// 轮询也起不来（目录没有、线程开不了）：原因。
pub fn watch_dir(dir: &Path, on_change: impl Fn(&Path) + Send + 'static) -> Result<Watch, String> {
    begin(Table::of_dir(dir), Backend::Native, QUIET, on_change)
}

/// 监视目录 `dir` 和它下面所有的子目录（施工 OV-1 下：覆盖、人格、预设、技能那几棵树）：哪一个文件变了（合并过的），交的是
/// 那个文件的路径（照 `dir` 拼的，不是真实的位置）；点开头的名字（路上哪一段都算）不算。目录要已经有了。别的同 [`watch`]。
///
/// # Errors
///
/// 轮询也起不来（目录没有、线程开不了）：原因。
pub fn watch_tree(dir: &Path, on_change: impl Fn(&Path) + Send + 'static) -> Result<Watch, String> {
    begin(Table::of_tree(dir), Backend::Native, QUIET, on_change)
}

/// 用哪一种看。
#[derive(Debug, Clone, Copy)]
pub(crate) enum Backend {
    /// 系统的监视，起不来的退回每 [`POLL`] 轮询一次。
    Native,
    /// 只轮询，隔这么久看一次（测试里走起不来的那条路）。
    Poll(Duration),
}

/// 照 `backend` 开始监视，`quiet` 里没有新的变动了才交出去。
pub(crate) fn start(
    files: &[PathBuf],
    backend: Backend,
    quiet: Duration,
    on_change: impl Fn(&Path) + Send + 'static,
) -> Result<Watch, String> {
    begin(Table::of(files), backend, quiet, on_change)
}

/// 照排好的表 `table` 开始监视。
pub(crate) fn begin(
    table: Table,
    backend: Backend,
    quiet: Duration,
    on_change: impl Fn(&Path) + Send + 'static,
) -> Result<Watch, String> {
    let (send, events) = channel();
    let (watcher, unavailable) = watcher(&table, backend, send)?;
    spawn(events, table, quiet, on_change)?;
    Ok(Watch {
        _watcher: watcher,
        unavailable,
    })
}

/// 造看表里几个目录的那一个：系统的起不来的换成轮询，交回起不来的原因。
fn watcher(
    table: &Table,
    backend: Backend,
    send: Sender<notify::Result<Event>>,
) -> Result<(Box<dyn Watcher + Send>, Option<String>), String> {
    match backend {
        Backend::Poll(every) => {
            let config = notify::Config::default().with_poll_interval(every);
            let watcher = PollWatcher::new(send, config)
                .and_then(|watcher| table.watch(watcher))
                .map_err(|error| error.to_string())?;
            Ok((Box::new(watcher), None))
        }
        Backend::Native => {
            match RecommendedWatcher::new(send.clone(), notify::Config::default())
                .and_then(|watcher| table.watch(watcher))
            {
                Ok(watcher) => Ok((Box::new(watcher), None)),
                Err(error) => {
                    let (watcher, _) = watcher(table, Backend::Poll(POLL), send)
                        .map_err(|poll| format!("{error}; polling: {poll}"))?;
                    Ok((watcher, Some(error.to_string())))
                }
            }
        }
    }
}

/// 开监视的线程。
fn spawn(
    events: Receiver<notify::Result<Event>>,
    table: Table,
    quiet: Duration,
    on_change: impl Fn(&Path) + Send + 'static,
) -> Result<(), String> {
    std::thread::Builder::new()
        .name("miyu-watch".to_string())
        .spawn(move || run(&events, &table, quiet, &on_change))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// 要看的：哪个目录（真实的位置）里的哪个名字，是调用的一方的哪一份。
#[derive(Debug)]
pub(crate) struct Table {
    targets: Vec<Target>,
}

/// 一行。
#[derive(Debug)]
struct Target {
    dir: PathBuf,
    /// 只认这个名字；没有的是目录里点开头的以外哪个都认（[`Table::of_dir`]）。
    name: Option<OsString>,
    file: PathBuf,
    /// 往下递归看，交出去的是变了的那个文件（[`Table::of_tree`]）。
    tree: bool,
}

impl Target {
    /// 认不认目录里的这个名字。
    fn names(&self, name: &std::ffi::OsStr) -> bool {
        match &self.name {
            Some(own) => own == name,
            None => !name.to_string_lossy().starts_with('.'),
        }
    }
}

impl Table {
    /// 照要看的几份排出表：每一份看它所在的目录；是链接的，再看本体所在的目录。
    pub(crate) fn of(files: &[PathBuf]) -> Table {
        let mut targets = Vec::new();
        for file in files {
            let (Some(parent), Some(name)) = (file.parent(), file.file_name()) else {
                continue;
            };
            let dir = real(parent);
            targets.push(Target {
                dir: dir.clone(),
                name: Some(name.to_os_string()),
                file: file.clone(),
                tree: false,
            });
            if let Ok(body) = std::fs::canonicalize(file)
                && body != dir.join(name)
                && let (Some(parent), Some(name)) = (body.parent(), body.file_name())
            {
                targets.push(Target {
                    dir: parent.to_path_buf(),
                    name: Some(name.to_os_string()),
                    file: file.clone(),
                    tree: false,
                });
            }
        }
        Table { targets }
    }

    /// 看一整个目录 `dir`（真实的位置）：交出去的是 `dir`。
    pub(crate) fn of_dir(dir: &Path) -> Table {
        Table {
            targets: vec![Target {
                dir: real(dir),
                name: None,
                file: dir.to_path_buf(),
                tree: false,
            }],
        }
    }

    /// 看目录 `dir` 和它下面的每一层（真实的位置）：交出去的是变了的那个文件，照 `dir` 拼。
    pub(crate) fn of_tree(dir: &Path) -> Table {
        Table {
            targets: vec![Target {
                dir: real(dir),
                name: None,
                file: dir.to_path_buf(),
                tree: true,
            }],
        }
    }

    /// 让 `watcher` 看表里的每个目录，不递归。
    fn watch<W: Watcher>(&self, mut watcher: W) -> notify::Result<W> {
        let mut dirs: Vec<&Path> = self.targets.iter().map(|t| t.dir.as_path()).collect();
        dirs.sort_unstable();
        dirs.dedup();
        for dir in dirs {
            let mode = match self.targets.iter().any(|t| t.tree && t.dir == dir) {
                true => RecursiveMode::Recursive,
                false => RecursiveMode::NonRecursive,
            };
            watcher.watch(dir, mode)?;
        }
        Ok(watcher)
    }

    /// 表里全部的几份，照给的先后，不重。
    fn files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = Vec::new();
        for target in &self.targets {
            if !files.contains(&target.file) {
                files.push(target.file.clone());
            }
        }
        files
    }

    /// 这一个变动碰到了哪几份。
    pub(crate) fn touched(&self, event: &Event) -> Vec<PathBuf> {
        if event.need_rescan() {
            return self.files();
        }
        // 只读的动静不算：打开、读完关上。写完关上的算（有的系统只报这一下）。
        if let EventKind::Access(access) = event.kind
            && access != AccessKind::Close(AccessMode::Write)
        {
            return Vec::new();
        }
        let mut touched: Vec<PathBuf> = Vec::new();
        for path in &event.paths {
            // 往下递归看的（[`Table::of_tree`]）：在它下面、路上没有点开头的那一段的，交出去的是这个文件。还在的目录不算：里面的
            // 文件变了自己会报，目录本身的时间、属性变了（Windows 读写里面的文件时会单独报一下）不是内容变了。
            for target in self.targets.iter().filter(|target| target.tree) {
                let real_path = real(path);
                if real_path.is_dir() {
                    continue;
                }
                let under = path
                    .strip_prefix(&target.dir)
                    .ok()
                    .or_else(|| real_path.strip_prefix(&target.dir).ok());
                if let Some(rest) = under
                    && !rest.as_os_str().is_empty()
                    && !rest
                        .components()
                        .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
                {
                    let file = target.file.join(rest);
                    if !touched.contains(&file) {
                        touched.push(file);
                    }
                }
            }
            let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
                continue;
            };
            let mut dir: Option<PathBuf> = None;
            for target in self
                .targets
                .iter()
                .filter(|target| !target.tree && target.names(name))
            {
                let same =
                    parent == target.dir || *dir.get_or_insert_with(|| real(parent)) == target.dir;
                if same && !touched.contains(&target.file) {
                    touched.push(target.file.clone());
                }
            }
        }
        touched
    }
}

/// 真实的位置；换不成的（没有、读不了）照原样。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 监视的线程：收变动，记下碰到的每一份最后一次动的时刻，`quiet` 里没有新的变动了交出去。送事件的那一头没了就退。
pub(crate) fn run(
    events: &Receiver<notify::Result<Event>>,
    table: &Table,
    quiet: Duration,
    on_change: &dyn Fn(&Path),
) {
    let mut pending: Vec<(PathBuf, Instant)> = Vec::new();
    loop {
        let due = pending.iter().map(|(_, at)| *at + quiet).min();
        let received = match due {
            Some(due) => events.recv_timeout(due.saturating_duration_since(Instant::now())),
            None => events.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let touched: Vec<PathBuf> = match &received {
            Ok(Ok(event)) => table.touched(event),
            // 监视报错：当这几份都变了，重读一遍（第 6 条）。
            Ok(Err(_)) => table.files(),
            Err(RecvTimeoutError::Timeout) => Vec::new(),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let now = Instant::now();
        for file in touched {
            pending.retain(|(waiting, _)| *waiting != file);
            pending.push((file, now));
        }
        let (ready, waiting): (Vec<_>, Vec<_>) = pending
            .into_iter()
            .partition(|(_, at)| now.saturating_duration_since(*at) >= quiet);
        pending = waiting;
        for (file, _) in ready {
            on_change(&file);
        }
    }
}

#[cfg(test)]
mod tests;

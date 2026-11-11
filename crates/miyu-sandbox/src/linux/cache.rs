//! 核心里按规格缓存的规则集（施工 V-47，2026-11-03 项目主人批「规则集缓存带失效检查：做；常驻沙盒派生进程：不做」）：只有
//! Landlock 的那一路，同一份规格只建一次规则集，之后每条命令先照 [`print`] 记一份指纹，和建的时候的逐字节比，一样才把那个
//! 规则集交给助手（助手只收紧自己，`pass.rs`），不一样就当场重建。指纹不靠时间戳：每次都把一级级走进的目录重新列一遍、
//! 每条规则的路径重新查一遍是什么文件。
//!
//! - 只缓存能缓存的规格（[`cacheable`]）：没写 `read`（关起来的会话，规则少，照旧从头建）、有藏起来的（没有的从头建本来就只有几条）、`hidden`、`write`、`reveal` 都是绝对路径（相对的照助手的工作目录
//!   算，核心这里算不准）。
//! - 键是规格里会进这一层的几格（`hidden`、`write`、`reveal`，先后也算）；`sockets` 每条命令都不一样，不进这一层，助手
//!   每条命令另叠一小层（`rules.rs` 的 `Layer::Unix`）。有效用户、组记在指纹里。只在这个进程的内存里，不落盘。
//! - 最多 [`CAP`] 份，满了丢最久没用的；没用过 [`IDLE`] 的丢掉（看门的线程每 [`SWEEP`] 看一次，没有了就退出）：规则集
//!   拿着它放行的每个文件（内核里的引用），家目录里删掉的文件要等它丢了才真正释放空间。
//! - 指纹记不成、建不成、建的时候路径变了的：这一次不缓存，交回没有，调的一方照旧让助手从头建（报的错和以前一样）。

use std::os::fd::{AsRawFd, OwnedFd};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use crate::Spec;

use super::print::{Print, print};
#[cfg(test)]
use super::rules::Rule;
use super::rules::{self, Layer, Outcome};

/// 最多缓存几份规格。
pub(crate) const CAP: usize = 8;

/// 多久没用的丢掉。
pub(crate) const IDLE: Duration = Duration::from_secs(5 * 60);

/// 看门的线程多久看一次。
const SWEEP: Duration = Duration::from_secs(60);

/// 规格能不能缓存（见模块说明）。
pub(crate) fn cacheable(spec: &Spec) -> bool {
    spec.read.is_none()
        && !spec.hidden.is_empty()
        && spec
            .hidden
            .iter()
            .chain(&spec.write)
            .chain(&spec.reveal)
            .all(|path| path.is_absolute())
}

/// 缓存的键：规格里进缓存那一层的几格。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    hidden: Vec<PathBuf>,
    write: Vec<PathBuf>,
    reveal: Vec<PathBuf>,
}

impl Key {
    fn of(spec: &Spec) -> Key {
        Key {
            hidden: spec.hidden.clone(),
            write: spec.write.clone(),
            reveal: spec.reveal.clone(),
        }
    }
}

/// 缓存的一份。
struct Entry {
    key: Key,
    /// 建的时候的指纹。
    print: Vec<u8>,
    /// 规则集（`O_CLOEXEC`，号码不小于 3）。
    fd: Arc<OwnedFd>,
    /// 上次交出去的时刻。
    used: Instant,
    /// 建的时候加的规则和每条加得怎么样（测试照它和此刻从头建的比）。
    #[cfg(test)]
    rules: Arc<Vec<(Rule, Outcome)>>,
}

/// 交出去的一份。
pub(crate) struct Served {
    /// 规则集。
    pub(crate) fd: Arc<OwnedFd>,
    /// 这一次新建的（没缓存、或者缓存的过期了）。
    #[cfg(test)]
    pub(crate) built: bool,
    /// 建它时加的规则和每条加得怎么样。
    #[cfg(test)]
    pub(crate) rules: Arc<Vec<(Rule, Outcome)>>,
}

/// 一个缓存。核心里一个（[`ruleset`]），测试各建各的。
pub(crate) struct Cache {
    entries: Mutex<Vec<Entry>>,
}

impl Cache {
    pub(crate) const fn new() -> Cache {
        Cache {
            entries: Mutex::new(Vec::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 规格 `spec` 的规则集：缓存的指纹和此刻的一样就交它，不一样、没有就新建一份放进去。规格不能缓存、指纹记不成、建不成
    /// 的交回没有。记指纹、建规则集都不拿着锁。
    pub(crate) fn ruleset(&self, spec: &Spec) -> Option<Served> {
        if !cacheable(spec) {
            return None;
        }
        let key = Key::of(spec);
        let now = print(spec).ok()?;
        {
            let mut entries = self.lock();
            if let Some(entry) = entries
                .iter_mut()
                .find(|entry| entry.key == key && entry.print == now.bytes)
            {
                entry.used = Instant::now();
                return Some(Served {
                    fd: Arc::clone(&entry.fd),
                    #[cfg(test)]
                    built: false,
                    #[cfg(test)]
                    rules: Arc::clone(&entry.rules),
                });
            }
            entries.retain(|entry| entry.key != key);
        }
        let (fd, outcomes) = build(&now).ok()?;
        let fd = Arc::new(fd);
        #[cfg(test)]
        let rules = Arc::new(now.rules.iter().cloned().zip(outcomes).collect::<Vec<_>>());
        #[cfg(not(test))]
        drop(outcomes);
        let mut entries = self.lock();
        entries.retain(|entry| entry.key != key);
        if entries.len() >= CAP
            && let Some(oldest) = (0..entries.len()).min_by_key(|&at| entries[at].used)
        {
            entries.swap_remove(oldest);
        }
        let mut print = now.bytes;
        print.shrink_to_fit();
        entries.push(Entry {
            key,
            print,
            fd: Arc::clone(&fd),
            used: Instant::now(),
            #[cfg(test)]
            rules: Arc::clone(&rules),
        });
        Some(Served {
            fd,
            #[cfg(test)]
            built: true,
            #[cfg(test)]
            rules,
        })
    }

    /// 丢掉到 `now` 已经 [`IDLE`] 没用的，交回还剩几份。
    pub(crate) fn sweep(&self, now: Instant) -> usize {
        let mut entries = self.lock();
        entries.retain(|entry| now.saturating_duration_since(entry.used) < IDLE);
        entries.len()
    }

    /// 还有几份。
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.lock().len()
    }
}

/// 照指纹 `print` 建缓存那一层（[`Layer::Base`]）：每条规则加的时候查到的要和指纹里的对得上。交回规则集（`O_CLOEXEC`、号码
/// 不小于 3：起子进程时标准输入、输出、错误先占 0～2，不会把它盖掉）和每条加得怎么样。
fn build(print: &Print) -> Result<(OwnedFd, Vec<Outcome>), String> {
    let created = rules::ruleset(Layer::Base)?;
    let (created, outcomes) = rules::build(created, &print.rules, Some(&print.idents))?;
    let fd: Option<OwnedFd> = created.into();
    let fd = fd.ok_or_else(|| "landlock is not available: not enforced".to_string())?;
    let fd = if fd.as_raw_fd() < 3 {
        rustix::io::fcntl_dupfd_cloexec(&fd, 3).map_err(|error| error.to_string())?
    } else {
        fd
    };
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC)
        .map_err(|error| error.to_string())?;
    Ok((fd, outcomes))
}

/// 核心里的那一个缓存。
static CACHE: Cache = Cache::new();

/// 看门的线程在不在。
static SWEEPING: AtomicBool = AtomicBool::new(false);

/// 规格 `spec` 的规则集（核心里的那一个缓存，见模块说明）：这台机器的 Landlock 不能用、规格不能缓存、指纹记不成、建不成的交回
/// 没有，照旧让助手从头建。
pub(crate) fn ruleset(spec: &Spec) -> Option<Arc<OwnedFd>> {
    if !usable() {
        return None;
    }
    let served = CACHE.ruleset(spec)?;
    sweeper();
    Some(served.fd)
}

/// [`ruleset`] 会不会看文件系统（施工 V-50）：规格能缓存、这台机器的 Landlock 能用的，每次都要记一份指纹，可能还要建规则集。
pub(crate) fn looks(spec: &Spec) -> bool {
    cacheable(spec) && usable()
}

/// 这台机器的 Landlock 能不能用：只探一次。
fn usable() -> bool {
    static USABLE: OnceLock<bool> = OnceLock::new();
    *USABLE.get_or_init(|| rules::available().is_ok())
}

/// 起看门的线程（已经在的不再起）：每 [`SWEEP`] 丢掉一次没用过 [`IDLE`] 的，一份都不剩就退出，之后缓存了再起。起不来的不要紧：
/// 丢不掉的只是多占着，满了照样丢最久没用的。
fn sweeper() {
    if SWEEPING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("miyu-sandbox-sweep".into())
        .spawn(|| {
            loop {
                std::thread::sleep(SWEEP);
                if CACHE.sweep(Instant::now()) > 0 {
                    continue;
                }
                SWEEPING.store(false, Ordering::Release);
                // 刚放下的这一下有人放进来了、又以为还有人看着：再看一眼，有就接着看。
                if CACHE.sweep(Instant::now()) == 0 || SWEEPING.swap(true, Ordering::AcqRel) {
                    break;
                }
            }
        });
    if spawned.is_err() {
        SWEEPING.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests;

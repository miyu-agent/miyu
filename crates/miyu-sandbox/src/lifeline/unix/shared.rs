//! 长住的子进程共用的看门的（施工 LW-28，`internal/blueprint/core.md`「子进程随核心退出」第 5 条）：扩展、命令型 MCP 服务、
//! 语义模型小程序有计划地停时先关标准输入、等、再只杀进程本身，从来不整组杀，所以不用各占一组，一根生命线上共用一个组、一个
//! 看门的就够了（原来一个一个起，装了十个扩展加语义模型的机器上常驻十一个看门的）。命令不一样：超时、中断时要整组杀，仍然
//! 一条一个组（`unix.rs` 的 `watcher`）。
//!
//! - 头一次 `keep` 起一个看门的、自成一组；之后的都进这个组，不再起。
//! - 还有人拿着的时候看门的没了（被别人杀了）：下一次 `keep` 再起一个、`setpgid` 进同一个组（组里还有没收掉的子进程，组号就
//!   还是这一组的），组里原有的子进程在核心没了时照样被收掉。进不去的（组里已经一个进程都没有了，组号可能被别人占）另起一个
//!   新组。
//! - 拿着的都丢了（最后一个长住的子进程退出、收掉了），先整组杀掉（组里剩的是它们起的、留在组里的子进程，施工 CS-7），
//!   看门的跟着杀掉、收掉，不留僵尸进程；下一次 `keep` 另起一个新组。

use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use super::Watcher;

/// 共用的组：组号，和这一刻看着它的看门的（换过的进程号和组号不一样）。最后一个拿着它的丢了，看门的跟着丢（杀掉、收掉）。
#[derive(Debug)]
struct Group {
    id: u32,
    watcher: Mutex<Watcher>,
}

/// 一根生命线上共用的那个组：只记着、不拿着，没有人用时是空的。
#[derive(Debug, Default)]
pub(super) struct Slot(Mutex<Weak<Group>>);

/// 一个长住的子进程拿着的共用的组（`lifeline.rs` 的 `Kept` 里）。
#[derive(Debug)]
pub(in crate::lifeline) struct Share(Arc<Group>);

impl Share {
    /// 组号：子进程照它起（`process_group`）。
    pub(in crate::lifeline) fn group(&self) -> u32 {
        self.0.id
    }

    /// 这一刻看着这个组的看门的的进程号。只有测试问（`testkit` 特性）。
    #[cfg(any(test, feature = "testkit"))]
    pub(in crate::lifeline) fn watchdog(&self) -> u32 {
        lock(&self.0.watcher).child.id()
    }
}

impl Slot {
    /// 拿共用的组：有、看门的还在的直接用；看门的没了的照 `start` 再起一个进这个组，进不去的、还没有的照 `start(0)` 另起一组。
    /// `start` 交给它组号，起一个看门的（`0` 是自成一组）。
    pub(super) fn keep(&self, start: impl Fn(i32) -> io::Result<Watcher>) -> io::Result<Share> {
        let mut current = lock(&self.0);
        if let Some(group) = current.upgrade() {
            let kept = {
                let mut watcher = lock(&group.watcher);
                if watcher.gone() {
                    match i32::try_from(group.id)
                        .map_err(io::Error::other)
                        .and_then(&start)
                    {
                        Ok(again) => {
                            *watcher = again;
                            true
                        }
                        // 进不去：组里已经一个进程都没有了，组号可能被别人占了，另起一组。
                        Err(_) => false,
                    }
                } else {
                    true
                }
            };
            if kept {
                return Ok(Share(group));
            }
        }
        let watcher = start(0)?;
        let group = Arc::new(Group {
            id: watcher.group(),
            watcher: Mutex::new(watcher),
        });
        *current = Arc::downgrade(&group);
        Ok(Share(group))
    }
}

impl Drop for Group {
    /// 最后一个拿着的丢了（施工 CS-7）：拿着的长住的子进程都退出、收掉了，组里还剩的只有它们起的、留在组里的子进程（扩展被
    /// 杀了以后留下的孙进程这类）和看门的。先整组 `SIGKILL`，再照 [`Watcher`] 的丢法收掉看门的。原来只收看门的，剩下的没人
    /// 看着，核心停了、核心没了都不走，成了一直留着的孤儿。看门的这一刻还没被收（问的这一下没收掉它）才杀：它在组里，组号就
    /// 不会被别人占，不会杀错；已经没了、收掉了的不杀（组号可能已经是别人的）。
    fn drop(&mut self) {
        let watcher = self
            .watcher
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        if !watcher.gone() {
            kill_group(self.id);
        }
    }
}

/// `kill(-组号, SIGKILL)`：组里已经没有进程的、杀不了的都不管（看门的跟着 [`Watcher`] 丢掉时照旧杀、收）。
#[expect(
    clippy::let_underscore_must_use,
    reason = "组里已经空了的杀不了也不要紧：只为不留孤儿"
)]
fn kill_group(id: u32) {
    use rustix::process::{Pid, Signal, kill_process_group};
    if let Some(pid) = i32::try_from(id).ok().and_then(Pid::from_raw) {
        let _ = kill_process_group(pid, Signal::KILL);
    }
}

/// 锁上：拿着锁的一方 panic 了也照样拿（里面只有组号和看门的，没有改到一半的）。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

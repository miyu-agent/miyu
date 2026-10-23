//! 等命令结束（施工 4-8；挪出来、加上等人的，施工 M-7 上）：另起一个线程等进程，结局送进一根通道。
//!
//! 命令里 `miyu call` 调的工具在等人确认时，那几段不算时限（设计 `39-脚本调工具.md` MC4，`tools/shell.md`「结果」）：端口经
//! [`Wakes::hold`] 往同一根通道里送「在等」「等完了」，等的那一头一起收，不轮询。在等的时候不设时限，只等「等完了」或者
//! 进程结束；等完了，把这一段之前走掉的时间从剩下的时限里扣掉接着等。人停掉这次调用照旧整组杀（[`super::Guard`]），进程
//! 一结束这里就醒。

use std::io;
use std::process::{Child, ExitStatus};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use super::{Ending, Finished, Group, Handover, KILL_WAIT, Started, TARGET, Waited, done, lock};

/// 等它结束交回来：结局连同进程的句柄。句柄跟着送：Windows 上它还开着，编号就不会被别的进程拿去。
pub(in crate::shell) type Exited = mpsc::Receiver<(io::Result<ExitStatus>, Child)>;

/// 通道里的一样。
enum Wake {
    /// 进程结束了。
    Exited(io::Result<ExitStatus>, Child),
    /// 命令里调的工具开始等人确认（`true`）、等完了（`false`）。
    Held(bool),
}

/// 等一条命令的那根通道（施工 M-7 上）：起命令之前造好，「在等人」的口子先交给端口（[`Self::hold`]），命令起来以后交给
/// [`Started::wait`]、[`Started::wait_or_move`]。
pub(in crate::shell) struct Wakes {
    sender: mpsc::Sender<Wake>,
    wakes: mpsc::Receiver<Wake>,
}

/// 到时、结束、等的线程没了。
enum Woke {
    Exited(io::Result<ExitStatus>, Child),
    TimedOut,
    Gone,
}

impl Wakes {
    pub(in crate::shell) fn new() -> Wakes {
        let (sender, wakes) = mpsc::channel();
        Wakes { sender, wakes }
    }

    /// 交给端口的「在等人」的口子（[`miyu_tool::NestedPort::hold`]）。等的这一头不等了的，送不进去也不要紧。
    pub(in crate::shell) fn hold(&self) -> miyu_tool::Hold {
        let sender = self.sender.clone();
        Box::new(move |held| {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "等的那一头不等了：这条命令已经结束"
            )]
            let _ = sender.send(Wake::Held(held));
        })
    }

    /// 另起一个线程等 `child` 结束，结局送进通道。
    fn watch(&self, mut child: Child) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            let status = child.wait();
            #[expect(
                clippy::let_underscore_must_use,
                reason = "等的那一头走了，结局就没人要了"
            )]
            let _ = sender.send(Wake::Exited(status, child));
        });
    }

    /// 等结束，最多等 `timeout`：等人确认的那几段不算。
    fn until(&self, timeout: Duration) -> Woke {
        let mut left = timeout;
        let mut held = false;
        loop {
            let since = Instant::now();
            let wake = match held {
                true => self
                    .wakes
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected),
                false => self.wakes.recv_timeout(left),
            };
            match wake {
                Ok(Wake::Exited(status, child)) => return Woke::Exited(status, child),
                Ok(Wake::Held(now)) => {
                    if !held {
                        left = left.saturating_sub(since.elapsed());
                    }
                    held = now;
                }
                Err(RecvTimeoutError::Timeout) => return Woke::TimedOut,
                Err(RecvTimeoutError::Disconnected) => return Woke::Gone,
            }
        }
    }

    /// 杀了以后最多再等 `patience`：等人的不管（杀了，脚本那头也就不等了）。
    fn within(&self, patience: Duration) -> Woke {
        let deadline = Instant::now() + patience;
        loop {
            match self
                .wakes
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(Wake::Exited(status, child)) => return Woke::Exited(status, child),
                Ok(Wake::Held(_)) => {}
                Err(RecvTimeoutError::Timeout) => return Woke::TimedOut,
                Err(RecvTimeoutError::Disconnected) => return Woke::Gone,
            }
        }
    }

    /// 交出去接着跑的（施工 7-12）：之后只认结束，另起一个线程转进 [`Exited`]。
    fn exited(self) -> Exited {
        let (exited, waited) = mpsc::channel();
        let Wakes { sender, wakes } = self;
        drop(sender);
        thread::spawn(move || {
            for wake in wakes {
                if let Wake::Exited(status, child) = wake {
                    #[expect(
                        clippy::let_underscore_must_use,
                        reason = "等的那一头走了，结局就没人要了"
                    )]
                    let _ = exited.send((status, child));
                    return;
                }
            }
        });
        waited
    }
}

impl Started {
    /// 等它结束，最多等 `timeout`（等人确认的那几段不算，`wakes`）：到时整组杀掉。
    ///
    /// # Errors
    ///
    /// 等不了：系统报了错。
    pub(in crate::shell) fn wait(self, timeout: Duration, wakes: Wakes) -> io::Result<Finished> {
        self.wait_or_kill((timeout, wakes), KILL_WAIT, Group::kill)
    }

    /// 同 [`Self::wait`]，只是到时不杀：读到的那一段、之后的输出、进程交回去，接着跑（施工 7-12）。
    ///
    /// # Errors
    ///
    /// 同 [`Self::wait`]。
    pub(in crate::shell) fn wait_or_move(
        self,
        timeout: Duration,
        wakes: Wakes,
    ) -> io::Result<Waited> {
        let Started {
            child,
            group,
            output,
            read,
            _watch,
        } = self;
        wakes.watch(child);
        match wakes.until(timeout) {
            Woke::Exited(status, child) => {
                let ending = Ending::Exited(status?);
                Ok(Waited::Finished(done(
                    ending,
                    Some(child),
                    group,
                    &output,
                    &read,
                )))
            }
            Woke::TimedOut => {
                let (sender, rest) = mpsc::channel();
                let output = {
                    let mut sink = lock(&output);
                    sink.moved = Some(sender);
                    std::mem::take(&mut sink.capture)
                };
                Ok(Waited::Moved(Handover {
                    output,
                    rest,
                    process: crate::shell::moved::Moved::new(wakes.exited(), group, _watch),
                }))
            }
            Woke::Gone => Err(gone()),
        }
    }

    /// 同 [`Self::wait`]：到时用 `kill` 整组杀，杀了以后最多再等 `patience`，还不结束的也算超时，不再等它。测试里
    /// 换成杀不掉的、等得短的。
    pub(super) fn wait_or_kill(
        self,
        (timeout, wakes): (Duration, Wakes),
        patience: Duration,
        kill: impl FnOnce(Group),
    ) -> io::Result<Finished> {
        let Started {
            child,
            group,
            output,
            read,
            _watch,
        } = self;
        wakes.watch(child);
        let (ending, child) = match wakes.until(timeout) {
            Woke::Exited(status, child) => (Ending::Exited(status?), Some(child)),
            Woke::TimedOut => {
                kill(group);
                match wakes.within(patience) {
                    Woke::Exited(status, child) => {
                        status?;
                        (Ending::TimedOut, Some(child))
                    }
                    Woke::TimedOut => {
                        tracing::warn!(target: TARGET, "command still running after kill");
                        (Ending::TimedOut, None)
                    }
                    Woke::Gone => return Err(gone()),
                }
            }
            Woke::Gone => return Err(gone()),
        };
        Ok(done(ending, child, group, &output, &read))
    }
}

/// 等的线程没了。
fn gone() -> io::Error {
    io::Error::other("the thread waiting for the command stopped")
}

#[cfg(all(test, unix))]
mod tests;

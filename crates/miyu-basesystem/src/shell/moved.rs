//! 到时交出去接着跑的前台命令（施工 7-12，`docs/blueprint/tools/shell.md`「怎么走」第 6 条）：和一开始就放到后台的
//! （`background.rs`）是同一种任务，交给任务表的样子一样。不同的是进程早就起了、有人在等它（[`process`] 里等的线程），
//! 输出早就在读：到这一刻读到的先交，之后的从读的线程的通道里接着交。

use std::io;
use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};

use miyu_tool::{Exit, Process};

use super::background::exit;
use super::output::Crlf;
use super::process::{self, Exited, Group};

/// 交出去的进程：等的线程还在等，结局从它那里拿。
pub(in crate::shell) struct Moved {
    /// 等它结束的那一头拿走。
    exited: Mutex<Option<Exited>>,
    group: Group,
    /// 等到了、句柄就要放下：之后不再按编号杀（同 `background.rs`）。
    ended: Mutex<bool>,
    /// 组里看门的（施工 7-8）：丢掉这条命令时收掉它。
    _watch: process::Watch,
}

impl Moved {
    /// 等的线程在 `exited` 那一头交结局，整组杀时认 `group`。
    pub(super) fn new(exited: Exited, group: Group, watch: process::Watch) -> Moved {
        Moved {
            exited: Mutex::new(Some(exited)),
            group,
            ended: Mutex::new(false),
            _watch: watch,
        }
    }
}

impl Process for Moved {
    fn wait(&self) -> io::Result<Exit> {
        let exited = lock(&self.exited)
            .take()
            .ok_or_else(|| io::Error::other("the command was already waited for"))?;
        let (status, child) = exited
            .recv()
            .map_err(|_| io::Error::other("the thread waiting for the command stopped"))?;
        *lock(&self.ended) = true;
        drop(child);
        let status = status?;
        self.group.leftovers();
        Ok(exit(status))
    }

    fn kill(&self) {
        let ended = lock(&self.ended);
        if !*ended {
            self.group.kill();
        }
    }
}

/// 交出去的输出：先交到那一刻读到的（`first`），再一段段接着交读的线程送来的，换好行尾；读的线程读完了关掉通道，
/// 留着的 `\r` 交回来就没有了。
pub(in crate::shell) struct Rest {
    first: Option<String>,
    rest: mpsc::Receiver<String>,
    crlf: Crlf,
    ended: bool,
}

impl Rest {
    /// 先交 `first`，再接着交 `rest` 送来的。
    pub(in crate::shell) fn new(first: String, rest: mpsc::Receiver<String>) -> Rest {
        Rest {
            first: (!first.is_empty()).then_some(first),
            rest,
            crlf: Crlf::default(),
            ended: false,
        }
    }
}

impl Iterator for Rest {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        if let Some(first) = self.first.take() {
            return Some(first);
        }
        while !self.ended {
            match self.rest.recv() {
                Ok(text) => {
                    let text = self.crlf.push(&text);
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
                Err(_) => self.ended = true,
            }
        }
        let rest = self.crlf.finish();
        (!rest.is_empty()).then_some(rest)
    }
}

/// 读锁：拿着它的线程 panic 了也照样拿。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

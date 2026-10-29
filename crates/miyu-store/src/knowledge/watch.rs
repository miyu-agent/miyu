//! 建索引的进度（施工 W-15，网页要的：重建索引有进度）：照文件补一个库时（[`Knowledge::caught`]，人重建、列出来、搜、挂目录、
//! 她用工具，都走这一条）要重建的文档一篇篇报给登记的那一头，完了报一声。没有要重建的不报。
//!
//! 报得太密没用：同一个库最多 [`EVERY`] 报一次，开头、最后一篇照报。向量（照意思找的那一路）在搜过以后后台补，不算在这里。
//!
//! [`Knowledge::caught`]: super::Knowledge

use std::sync::Arc;
use std::time::{Duration, Instant};

/// 同一个库两次报之间至少隔多久。
const EVERY: Duration = Duration::from_millis(100);

/// 收进度的那一头：核心照它推给头。
pub trait IndexWatch: Send + Sync {
    /// 账号 `account` 的库 `kb` 在建：要重建的 `total` 篇里做完了 `done` 篇。
    fn indexing(&self, kb: &str, done: usize, total: usize);
    /// 账号 `account` 的库 `kb` 建好了。
    fn indexed(&self, kb: &str);
}

/// 登记的那一头（`Knowledge` 要 `Debug`）。
#[derive(Clone)]
pub(super) struct Watcher(pub(super) Arc<dyn IndexWatch>);

impl std::fmt::Debug for Watcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Watcher")
    }
}

/// 补一个库这一趟的进度：开头报一次，之后照 [`EVERY`] 疏一疏，最后一篇照报。
pub(super) struct Progress<'a> {
    watcher: Option<&'a Watcher>,
    kb: &'a str,
    done: usize,
    total: usize,
    last: Instant,
}

impl<'a> Progress<'a> {
    /// 要重建 `total` 篇：有的当场报开头。
    pub(super) fn start(watcher: Option<&'a Watcher>, kb: &'a str, total: usize) -> Progress<'a> {
        let progress = Progress {
            watcher,
            kb,
            done: 0,
            total,
            last: Instant::now(),
        };
        if let (Some(watcher), true) = (progress.watcher, total > 0) {
            watcher.0.indexing(kb, 0, total);
        }
        progress
    }

    /// 又做完一篇。
    pub(super) fn one(&mut self) {
        self.done += 1;
        let Some(watcher) = self.watcher else {
            return;
        };
        if self.done == self.total || self.last.elapsed() >= EVERY {
            self.last = Instant::now();
            watcher.0.indexing(self.kb, self.done, self.total);
        }
    }

    /// 补完了：重建过的报一声建好了。
    pub(super) fn finish(&self) {
        if let (Some(watcher), true) = (self.watcher, self.total > 0) {
            watcher.0.indexed(self.kb);
        }
    }

    /// 这一趟有没有重建过。
    pub(super) fn rebuilt(&self) -> bool {
        self.total > 0
    }
}

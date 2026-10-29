//! 建索引的进度推给头（施工 W-15，网页的知识库页要的：重建索引有进度）：知识库照文件补的时候报进来（`miyu_store` 的
//! `IndexWatch`），这里写成推送的一行广播出去，订阅着 `kb` 的连接只推自己账号的（`subscriptions/listed.rs`）。
//!
//! - `kb.indexing {kb, done, total}`：要重建的 `total` 篇里做完了 `done` 篇；开头是 0，最后一篇照推，中间至多 0.1 秒一次。
//! - `kb.indexed {kb}`：建好了。头照 `kb.list` 重读那一个库的样子（`indexed_at`、数）。
//!
//! 推的都是这一刻的样子，掉了队的推 `resync`，头重新订阅、重读 `kb.list` 就对了。

use serde_json::json;
use tokio::sync::broadcast;

use miyu_store::knowledge::IndexWatch;

/// 广播里最多攒几条没收的：报得疏，一个库建一次几十条以内。
const PUSHES: usize = 256;

/// 建索引的进度的广播：一条是「账号、换行、推送的那一行」。
#[derive(Debug)]
pub(crate) struct Progress(broadcast::Sender<String>);

impl Default for Progress {
    fn default() -> Progress {
        Progress(broadcast::channel(PUSHES).0)
    }
}

impl Progress {
    /// 收进度的一头。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<String> {
        self.0.subscribe()
    }

    fn send(&self, line: &serde_json::Value) {
        if self.0.send(line.to_string()).is_err() {
            // 没有订阅着的连接：不用说。
        }
    }
}

impl IndexWatch for Progress {
    fn indexing(&self, kb: &str, done: usize, total: usize) {
        let line = json!({"jsonrpc": "2.0", "method": "kb.indexing",
            "params": {"kb": kb, "done": done, "total": total}});
        self.send(&line);
    }

    fn indexed(&self, kb: &str) {
        let line = json!({"jsonrpc": "2.0", "method": "kb.indexed", "params": {"kb": kb}});
        self.send(&line);
    }
}

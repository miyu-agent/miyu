//! 「哪个项目变了」的广播（施工 H-1）：`project.create`、`update`、`delete` 写成了报一声，推给订阅着 `projects` 的连接
//! （`subscriptions/listed.rs`）。推的是这一刻的整项，晚到的、重复的都对。一条是项目编号：和软件包、扩展的
//! 推送共用一个转发任务，那边收的是字。

use tokio::sync::broadcast;

/// 广播里最多攒几条没收的：人一次只改一个项目。
const CHANGES: usize = 64;

/// 项目变了的广播。
#[derive(Debug)]
pub(crate) struct Changes(broadcast::Sender<String>);

impl Default for Changes {
    fn default() -> Changes {
        Changes(broadcast::channel(CHANGES).0)
    }
}

impl Changes {
    /// 收「哪个项目变了」的一头。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<String> {
        self.0.subscribe()
    }

    /// 广播项目 `id` 变了。
    pub(crate) fn notify(&self, id: &str) {
        if self.0.send(id.to_string()).is_err() {
            // 没有订阅着的连接：不用说。
        }
    }
}

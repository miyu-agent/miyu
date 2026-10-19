//! 「哪个账号的哪个项目变了」的广播（施工 H-1）：`project.create`、`update`、`delete` 写成了报一声，订阅着 `projects` 的连接只推
//! 自己账号的（`subscriptions/listed.rs`）。推的是这一刻的整项，晚到的、重复的都对。一条是「账号、换行、编号」：和软件包、扩展的
//! 推送共用一个转发任务，那边收的是字。

use tokio::sync::broadcast;

use miyu_kernel::id::AccountId;

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

    /// 广播 `account` 的项目 `id` 变了。
    pub(crate) fn notify(&self, account: &AccountId, id: &str) {
        if self.0.send(format!("{}\n{id}", account.as_str())).is_err() {
            // 没有订阅着的连接：不用说。
        }
    }
}

/// 一条是不是 `account` 的：是的交回项目编号，别的账号的、写法不对的没有。
pub(crate) fn mine<'a>(changed: &'a str, account: &AccountId) -> Option<&'a str> {
    let (whose, project) = changed.split_once('\n')?;
    (whose == account.as_str()).then_some(project)
}

#[cfg(test)]
mod tests;

//! 给人的通知（施工 J-3 下，设计 `45-定时和循环.md` 第三节第 2 条、第五节第 3 条；形状两个头 2026-10-27 都推荐）：账号级的
//! `notices` 流，头照它弹系统通知。一条是「账号、换行、推送的那一行」，订阅着 `notices` 的连接只推自己账号的
//! （`subscriptions/listed.rs`，同知识库的进度）。不存盘：没订着的连接收不到，重连也不补。
//!
//! - `schedule.ran {schedule, session, ok}`：每次新开会话的定时，开出来的那一轮结束了（`ran.rs`）；`ok` 是答完了。
//! - `schedules.reset {}`：这个账号的唤醒表坏了，改名留着、换了一份空的，原来的都没了。

use serde_json::{Value, json};
use tokio::sync::broadcast;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;

/// 广播里最多攒几条没收的：到点的一次跑完几个。
const PUSHES: usize = 64;

/// 通知的广播。克隆的是同一个。
#[derive(Debug, Clone)]
pub(crate) struct Notices(broadcast::Sender<String>);

impl Default for Notices {
    fn default() -> Notices {
        Notices(broadcast::channel(PUSHES).0)
    }
}

impl Notices {
    /// 收通知的一头。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<String> {
        self.0.subscribe()
    }

    /// 给 `account` 推一条：`kind`、`at`，再并上 `more` 的几格。
    pub(crate) fn send(&self, account: &AccountId, kind: &str, at: Timestamp, more: Value) {
        let mut params = json!({"kind": kind, "at": at});
        if let (Some(params), Value::Object(more)) = (params.as_object_mut(), more) {
            params.extend(more);
        }
        let line = json!({"jsonrpc": "2.0", "method": "notice", "params": params});
        if self
            .0
            .send(format!("{}\n{line}", account.as_str()))
            .is_err()
        {
            // 没有订阅着的连接：不用说。
        }
    }
}

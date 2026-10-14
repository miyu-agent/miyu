//! 有没有人在用：连着几个连接（施工 3-9 上，[`Connected`]），和「来请求了、连上了、断开了」的信号（施工 V-5 中、V-6 下，
//! [`Activity`]）。

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::watch;

use crate::Core;

/// 来请求了、连上了、断开了：连接每读到一个请求叫一声（`connection.rs`），连上、断开各叫一声（`Connected`，施工 V-6
/// 下）。核心照它在一阵请求过去、安静下来以后把堆里空着的还给系统（`miyu-core` 的 `settle.rs`），照它算空闲退出（`serve.rs`）：
/// 只在有动静时叫醒等着的，平时谁都不醒。是个计数：几处各拿一份 [`Activity::subscribe`]，各自都叫得醒。
#[derive(Debug)]
pub struct Activity(watch::Sender<u64>);

impl Default for Activity {
    fn default() -> Activity {
        Activity(watch::Sender::new(0))
    }
}

impl Activity {
    /// 叫一声：计数加一，各处等着的都醒。
    pub fn touch(&self) {
        self.0.send_modify(|count| *count = count.wrapping_add(1));
    }

    /// 拿一份来等：`changed()` 等下一声；拿到以前叫过的不算。
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.0.subscribe()
    }
}

/// 连着的一个连接：数着，走的时候减掉，连接的任务被叫停了也减。
pub(crate) struct Connected(Arc<Core>);

impl Connected {
    /// 数上一个。
    pub(crate) fn new(core: Arc<Core>) -> Connected {
        core.connections.fetch_add(1, Ordering::AcqRel);
        core.activity.touch();
        Connected(core)
    }
}

/// 走的时候减掉，再叫一声（施工 V-6 下）：空闲退出照它重新算，不隔一阵再看。
impl Drop for Connected {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::AcqRel);
        self.0.activity.touch();
    }
}

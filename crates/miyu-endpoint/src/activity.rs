//! 有没有人在用：连着几个连接（施工 3-9 上，[`Connected`]），和「来请求了、连上了、断开了」的信号（施工 V-5 中、V-6 下，
//! [`Activity`]）；头叫核心停下的信号也在这里（施工 DT-4 下，`core.stop`），核心的进程照它停。

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::watch;

use crate::Core;

/// 来请求了、连上了、断开了：连接每读到一个请求叫一声（`connection.rs`），连上、断开各叫一声（`Connected`，施工 V-6
/// 下）。核心照它在一阵请求过去、安静下来以后把堆里空着的还给系统（`miyu-core` 的 `settle.rs`），照它算空闲退出（`serve.rs`）：
/// 只在有动静时叫醒等着的，平时谁都不醒。是个计数：几处各拿一份 [`Activity::subscribe`]，各自都叫得醒。
#[derive(Debug)]
pub struct Activity {
    /// 叫了几声。
    touched: watch::Sender<u64>,
    /// 有没有头叫停（施工 DT-4 下）：只会从没有变成有。
    stop: watch::Sender<bool>,
}

impl Default for Activity {
    fn default() -> Activity {
        Activity {
            touched: watch::Sender::new(0),
            stop: watch::Sender::new(false),
        }
    }
}

impl Activity {
    /// 叫一声：计数加一，各处等着的都醒。
    pub fn touch(&self) {
        self.touched
            .send_modify(|count| *count = count.wrapping_add(1));
    }

    /// 拿一份来等：`changed()` 等下一声；拿到以前叫过的不算。
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.touched.subscribe()
    }

    /// 头叫核心停下（`core.stop`，施工 DT-4 下）：回应写出去以后才叫，叫过的再叫不变。
    pub(crate) fn ask_to_stop(&self) {
        self.stop.send_replace(true);
    }

    /// 等到有头叫停：叫过了的马上交回，没叫的一直等（`miyu-core` 的 `serve.rs` 照它停，原因是 `request`）。
    pub async fn stop_requested(&self) {
        let mut asked = self.stop.subscribe();
        // 发的一头就在 `self` 里，等的时候断不了；真断了也当没人叫停。
        if asked.wait_for(|asked| *asked).await.is_err() {
            std::future::pending::<()>().await;
        }
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

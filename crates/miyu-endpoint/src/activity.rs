//! 有没有人在用：连着几个连接（施工 3-9 上，[`Connected`]），和「来请求了」的信号（施工 V-5 中，[`Activity`]）。

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::Notify;

use crate::Core;

/// 来请求了：连接每读到一个请求叫一声（`connection.rs`）。核心照它在一阵请求过去、安静下来以后把堆里空着的还给系统
/// （`miyu-core` 的 `settle.rs`）：头一连上的那一阵（读配置、列模型、读人）放掉的几 MB 留在 glibc 手里，不还就一直算在
/// 核心头上。只在来请求时叫醒等着的，平时谁都不醒。
#[derive(Debug, Default)]
pub struct Activity(Notify);

impl Activity {
    /// 叫一声。没人在等的记下一次，等的一方下次一等就醒。
    pub fn touch(&self) {
        self.0.notify_one();
    }

    /// 等下一声。
    pub async fn wait(&self) {
        self.0.notified().await;
    }
}

/// 连着的一个连接：数着，走的时候减掉，连接的任务被叫停了也减。
pub(crate) struct Connected(Arc<Core>);

impl Connected {
    /// 数上一个。
    pub(crate) fn new(core: Arc<Core>) -> Connected {
        core.connections.fetch_add(1, Ordering::AcqRel);
        Connected(core)
    }
}

impl Drop for Connected {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

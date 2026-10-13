//! 叫停、掐掉在跑的调用（施工 9-8 六补从 `tools.rs` 挪出来，那边过了 500 行）。

use super::Tools;
use crate::TARGET;
use crate::lines::millis;
use miyu_kernel::id::CallId;

impl Tools {
    /// 叫它停（施工 4-9 再补一）：举起这次调用的旗，不掐任务。工具看旗，停在改之前或者做完，照常送回来。不在跑的，
    /// 什么都不做。
    pub(crate) fn stop(&mut self, call_id: CallId) {
        if let Some(running) = self.running.get_mut(&call_id) {
            running.stop.raise();
            running.stopping = true;
            // 在等回答的不等了（施工 D-2）：口子一丢，工具当没答收场。
            running.answer = None;
        }
    }

    /// 掐掉一次在跑的调用：举旗，掐掉跑它的任务。之后什么都不再报。叫它停过、还没交回来的，多记一行 `WARN`：它要是
    /// 后来改完了，这次改动不留效果。
    pub(crate) fn cancel(&mut self, call_id: CallId) {
        let Some(running) = self.running.remove(&call_id) else {
            return;
        };
        running.stop.raise();
        running.task.abort();
        let call = call_id.to_string();
        tracing::info!(
            target: TARGET,
            call = call.as_str(),
            took_ms = millis(running.started.elapsed()),
            "stopped"
        );
        if running.stopping {
            tracing::warn!(target: TARGET, call = call.as_str(), "cancelled while stopping");
        }
    }
}

//! 替身的等确认到时（施工 CD-1，`docs/blueprint/kernel/asking.md`「到时没人确认」）：回合开始交给内核的等多久；内核交出的截止
//! 时刻只记下、不计时，测试照记下的那一条送「到时了」。

use super::Stage;
use crate::event::Said;
use crate::id::CallId;
use crate::session::Input;
use crate::time::Timestamp;

/// 到时写给她的那一句：同执行器，照出厂的原文。
const EXPIRED: &str =
    include_str!("../../../../resources/core/tool-results/approval-timed-out.txt");

/// 替身记着的等确认到时。
#[derive(Default)]
pub(super) struct Expiries {
    /// 回合开始时交给内核的等多久，毫秒（`TurnStartHooksDone` 的 `confirm_ms`）：默认没有，一直等。
    pub(super) wait: Option<u64>,
    /// 内核交出的截止时刻，照先后：哪一刻、哪一次调用、脚本里调的序号。
    pub(super) asked: Vec<(Timestamp, CallId, Option<u32>)>,
}

impl Stage {
    /// 之后的回合等人确认最多等 `ms` 毫秒；没有的一直等。
    pub fn confirm_wait(&mut self, ms: Option<u64>) {
        self.expiries.wait = ms;
    }

    /// 内核交出的截止时刻，照先后：哪一刻、哪一次调用、脚本里调的序号。
    pub fn expiries(&self) -> &[(Timestamp, CallId, Option<u32>)] {
        &self.expiries.asked
    }

    /// 送记下的第 `k` 个截止时刻到时了：时钟拨到那一刻（已经过了的，就是现在）。
    ///
    /// # Panics
    ///
    /// 没有第 `k` 个。
    pub fn expire(&mut self, k: usize) {
        let (at, call_id, nested) = self.expiries.asked[k];
        if at > self.now {
            self.now = at;
        }
        let input = Input::ApprovalExpired {
            at: self.now,
            call_id,
            nested,
            text: EXPIRED.trim_end_matches(['\n', '\r']).to_string(),
            human: Some(Said::new("core/tool-results/approval-timed-out")),
        };
        self.run(input);
    }
}

//! 定时任务的拒绝（施工 J-1 下，`docs/blueprint/schedules.md` 第五节）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 这个账号的唤醒表里没有这个编号。
    pub(crate) const UNKNOWN_SCHEDULE: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_schedule",
        data: None,
    };

    /// 参数不对，`data.problem` 说是哪一样：`when`（几种写法没写对一种）、`cron`（读不成）、`past`（时刻已经过了）、`short`
    /// （隔得少于 60 秒）、`prompt`（空的）、`session`（没有、不是他的）、`target`（两种都写了、都没写）。
    pub(crate) fn bad_schedule(problem: &str) -> Refusal {
        let mut data = serde_json::Map::new();
        data.insert("problem".to_string(), serde_json::json!(problem));
        Refusal {
            data: Some(data),
            ..Refusal::BAD_PARAMS
        }
    }
}

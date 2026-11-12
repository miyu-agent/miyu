//! 内存不够不开会话的拒绝（施工 MEM-1，`docs/blueprint/protocol.md`「会话表」第 10 条）。

use std::borrow::Cow;

use serde_json::{Map, Value, json};

use super::{REFUSED, Refusal};
use miyu_heap::room::Room;

/// 原因码。
const REASON: &str = "low_memory";

impl Refusal {
    /// 可用内存低于门槛，没开会话（新造、载入、造子会话都算）：`data.available_mib` 还剩多少 MiB，`data.available_percent`
    /// 占总量的百分之几（都往下取整）。
    pub(crate) fn low_memory(room: &Room) -> Refusal {
        let mut data = Map::new();
        data.insert("available_mib".to_string(), json!(room.mib()));
        data.insert("available_percent".to_string(), json!(room.percent()));
        Refusal {
            code: REFUSED,
            reason: REASON,
            data: Some(data),
        }
    }
}

/// `low_memory` 照 `data` 里的两个数说的中文、英文；不是它、`data` 里少了数的没有（照 `message::of` 的那一句）。
pub(super) fn said(
    reason: &str,
    data: Option<&Map<String, Value>>,
) -> Option<(Cow<'static, str>, Cow<'static, str>)> {
    if reason != REASON {
        return None;
    }
    let number = |name: &str| data?.get(name)?.as_u64();
    let (mib, percent) = (number("available_mib")?, number("available_percent")?);
    Some((
        format!("可用内存只剩 {mib} MB（{percent}%），暂时不能打开会话。请关掉一些程序后重试。").into(),
        format!("Only {mib} MB ({percent}%) of memory available. Can't open a session now. Free some memory and try again.").into(),
    ))
}

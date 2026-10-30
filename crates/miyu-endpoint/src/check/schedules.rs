//! 唤醒表里读不懂的行（施工 AE-4）：某一格被改坏了、换了这一版不认的写法的，到点、列出来都跳过，别的照常。一行一条
//! 警告 `schedule_unreadable`，`file` 是唤醒表，`message` 带编号和为什么；人照编号删（`miyu schedule cancel`）。

use serde_json::{Value, json};

use miyu_store::human::Human;

use super::shown;
use crate::Core;

/// 几条警告：`kind` 是 `schedule`。表小，照拿着连接的那把锁读一遍，不另开线程。
pub(super) fn unreadable(core: &Core, words: &Human) -> Vec<Value> {
    let (rows, path) = core.schedules().unreadable();
    let file = shown(core, &path);
    rows.iter()
        .map(|one| {
            let detail = format!("{}: {}", one.id, one.why);
            let message = crate::packages::sentence(words, "schedule_unreadable", &detail);
            json!({"kind": "schedule", "file": file, "code": "schedule_unreadable", "level": "warning", "message": message.unwrap_or_default()})
        })
        .collect()
}

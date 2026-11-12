//! 文件的修改时间换成内核的时刻（施工 SW-1，`docs/blueprint/tools/show.md`「怎么走」第 3 条）：`show` 记进 `file.shown`，
//! `session.shown` 读之前照它核对文件变没变。两头照同一个函数算，精度同事件的时刻（毫秒），不会因为取整不同对不上。

use std::fs::Metadata;
use std::time::{SystemTime, UNIX_EPOCH};

use miyu_kernel::time::Timestamp;

/// `metadata` 的修改时间：UTC，往早取整到毫秒。平台给不出修改时间的、落在 0000 年到 9999 年以外的没有。
pub fn modified_at(metadata: &Metadata) -> Option<Timestamp> {
    at(metadata.modified().ok()?)
}

/// 时刻 `time` 往早取整到毫秒：1970 年以后的截掉零头，以前的照绝对值往上取整再取负，两边都是往早取。
fn at(time: SystemTime) -> Option<Timestamp> {
    let millis = match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_millis()).ok()?,
        Err(before) => {
            let nanos = before.duration().as_nanos();
            -i64::try_from(nanos.div_ceil(1_000_000)).ok()?
        }
    };
    Timestamp::from_unix_millis(millis)
}

#[cfg(test)]
mod tests;

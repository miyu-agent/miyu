//! 日志中间坏了一行（施工 YC-6，`docs/construction/YC-6-日志坏一行只读打开.md`，压测报告 F6）：一页要读的行里碰到坏行的，照坏行
//! 以前的交这一页，带上坏在哪（`broken`），不再整页拒 `session_broken`，坏行以前的对话照样翻得到。日志一个字节都不动：不截、
//! 不修（2026-11-10 项目主人定：只读打开，不给修；要接着聊新开一个会话）。
//!
//! 怎么少读：坏行是第几条照段名、行号算（[`OpenError::broken_at`]），拿它当 `before` 再读一遍；那一截里又碰到更早的坏行的，再往前
//! 收，`before` 只会变小，一定停得下。坏在第一条的（连造会话那一条都读不成）、算不出更早的（段名接不上、算出来的不比这一回的
//! `before` 小），照旧拒：没有能看的。

use std::path::Path;

use miyu_kernel::event::Event;
use miyu_store::log::{BrokenAt, OpenError};

use super::read::{self, Slice};

/// 同 [`read::read`]，只是要读的行里有坏行的，照坏行以前的读、[`Slice`] 记下坏在哪（照它停的那一处）。没坏的一样，一个字节都不差。
///
/// # Errors
///
/// 同 [`read::read`]；坏在第一条的、算不出更早的坏行的照旧报坏了。
pub(super) fn read(
    dir: &Path,
    before: Option<u64>,
    limits: (usize, usize),
    learn: bool,
    size: &impl Fn(&Event) -> usize,
) -> Result<Slice, OpenError> {
    let mut before = before;
    let mut broken: Option<BrokenAt> = None;
    loop {
        match read::read(dir, before, limits, learn, size) {
            Ok(mut slice) => {
                slice.broken = broken;
                return Ok(slice);
            }
            Err(error) => {
                let at = error
                    .broken_at()
                    .filter(|at| at.seq.get() > 1 && before.is_none_or(|b| at.seq.get() < b));
                let Some(at) = at else {
                    return Err(error);
                };
                before = Some(at.seq.get());
                broken = Some(at);
            }
        }
    }
}

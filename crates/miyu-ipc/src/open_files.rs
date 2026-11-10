//! 打开文件数的软上限（施工 YC-1，压测报告 F1）：系统给的软上限常是 1024，几百个会话就用光，核心、网页、im 起来时调高。
//!
//! 调到硬上限，最多 [`CAP`]：硬上限是无限的（macOS 常这样）、大得离谱的（容器里有十亿的）不照它。调不上去的退到
//! [`FALLBACK`]（macOS 的 `OPEN_MAX`，软上限过了 `kern.maxfilesperproc` 时调不上去）。拉起的子进程跟着继承调高的值，不还原：
//! 用 `select` 的老程序只有自己开过 1024 个文件才会出事，上限封在 [`CAP`] 也不让挨个关描述符的老程序变慢。Windows 上没有
//! 这个上限，什么都不做。

use std::io;

/// 调到的最高值：压测 1600 个会话同时干活用了约 5200 个。
pub const CAP: u64 = 65_536;

/// 调到 [`CAP`] 不成时退到的值：macOS 的 `OPEN_MAX`。
pub const FALLBACK: u64 = 10_240;

/// 调过的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raised {
    /// 已经够了，没动：当前的软上限（`None` 是无限）。
    Enough(Option<u64>),
    /// 从 `from` 调到了 `to`。
    To {
        /// 原来的软上限。
        from: u64,
        /// 调到的。
        to: u64,
    },
}

/// 把这个进程的打开文件数软上限调高（模块的说明），调了的记一行 `DEBUG`。起别的线程之前调，免得别处同时在开文件。
///
/// # Errors
///
/// 两个值都调不上去：交回最后一次的错误。
pub fn raise() -> io::Result<Raised> {
    let raised = sys::raise()?;
    if let Raised::To { from, to } = raised {
        tracing::debug!(target: "miyu::ipc", from, to, "open files raised");
    }
    Ok(raised)
}

/// 照当前的软上限 `current`、硬上限 `maximum`（`None` 是无限）要调到的值，依次试；够了的是空的。纯逻辑，只有 Unix 用。
#[cfg(unix)]
fn targets(current: Option<u64>, maximum: Option<u64>) -> Vec<u64> {
    let Some(current) = current else {
        return Vec::new();
    };
    let top = maximum.map_or(CAP, |maximum| maximum.min(CAP));
    let mut wanted = Vec::new();
    for target in [top, FALLBACK.min(top)] {
        if target > current && !wanted.contains(&target) {
            wanted.push(target);
        }
    }
    wanted
}

#[cfg(unix)]
mod sys {
    use std::io;

    use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

    use super::{Raised, targets};

    pub(super) fn raise() -> io::Result<Raised> {
        let now = getrlimit(Resource::Nofile);
        let mut last = None;
        for target in targets(now.current, now.maximum) {
            let wanted = Rlimit {
                current: Some(target),
                maximum: now.maximum,
            };
            match setrlimit(Resource::Nofile, wanted) {
                Ok(()) => {
                    return Ok(Raised::To {
                        from: now.current.unwrap_or(target),
                        to: target,
                    });
                }
                Err(error) => last = Some(io::Error::from(error)),
            }
        }
        last.map_or(Ok(Raised::Enough(now.current)), Err)
    }
}

#[cfg(not(unix))]
mod sys {
    use std::io;

    use super::Raised;

    pub(super) fn raise() -> io::Result<Raised> {
        Ok(Raised::Enough(None))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{CAP, FALLBACK, targets};

    #[test]
    fn the_usual_linux_limits_go_up_to_the_cap() {
        assert_eq!(targets(Some(1024), Some(524_288)), [CAP, FALLBACK]);
    }

    #[test]
    fn an_unlimited_maximum_is_capped() {
        assert_eq!(targets(Some(256), None), [CAP, FALLBACK]);
    }

    #[test]
    fn a_small_maximum_is_the_only_target() {
        assert_eq!(targets(Some(1024), Some(4096)), [4096]);
    }

    #[test]
    fn a_maximum_between_the_fallback_and_the_cap_is_tried_first() {
        assert_eq!(targets(Some(1024), Some(20_000)), [20_000, FALLBACK]);
    }

    #[test]
    fn enough_already_is_left_alone() {
        assert!(targets(Some(CAP), Some(524_288)).is_empty());
        assert!(targets(Some(100_000), None).is_empty());
        assert!(targets(None, None).is_empty());
    }

    #[test]
    fn above_the_fallback_only_the_cap_is_tried() {
        assert_eq!(targets(Some(20_000), Some(524_288)), [CAP]);
    }
}

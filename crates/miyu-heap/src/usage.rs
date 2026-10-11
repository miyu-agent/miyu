//! 进程这时占了多少内存（`docs/designs/23-性能预算.md` F4，施工 V-4）：核心定时采样自己记账用。Linux 上读
//! `/proc/self/smaps_rollup`（采样那一次）、`/proc/self/status`（中间看一眼涨没涨，便宜）；macOS 上问内核 `proc_pid_rusage`
//! （施工 V-4 补：RSS 和活动监视器里的「内存」那一列）；Windows 还没定怎么量，交回没有。

/// 进程这时占的内存，字节。`rss` 哪个平台都有；别的几样只有量得出的平台有（Linux 有 `pss`、`private`、`anon`、`swap`，
/// macOS 有 `footprint`），没有的是没有。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    /// 在物理内存里的（RSS）：和别的进程共用的页整份算。
    pub rss: u64,
    /// 照分摊算的（PSS，Linux）：和几个进程共用的页只算几分之一。
    pub pss: Option<u64>,
    /// 只有这个进程用的页（Linux：`Private_Clean` 加 `Private_Dirty`）。
    pub private: Option<u64>,
    /// 匿名的页（Linux）：堆、栈这些，不是文件映射进来的。
    pub anon: Option<u64>,
    /// 换出到交换区的（Linux）。
    pub swap: Option<u64>,
    /// 算在这个进程头上的（macOS 的 `phys_footprint`）：活动监视器「内存」那一列，压缩了的也算。
    pub footprint: Option<u64>,
}

/// 这时的用量：Linux 上读 `/proc/self/smaps_rollup`（读不了的，内核太老、没挂 `/proc`，交回没有）；macOS 上问内核；Windows
/// 交回没有。Linux 上要走一遍进程的映射，在阻塞线程里调。
pub fn usage() -> Option<Usage> {
    imp::usage()
}

/// 这时的 RSS：Linux 上读 `/proc/self/status` 的 `VmRSS`，比 [`usage`] 便宜，看涨没涨用；macOS 上问内核；Windows 交回没有。
pub fn resident() -> Option<u64> {
    imp::resident()
}

/// 读 `smaps_rollup` 的内容：每行 `<名字>: <数> kB`，认 `Rss`、`Pss`、`Private_Clean`、`Private_Dirty`、`Anonymous`、
/// `Swap` 这几行，别的不看。一行 `Rss` 都没有的交回没有（读到的不是它）。
pub fn parse_rollup(text: &str) -> Option<Usage> {
    let mut rss = None;
    let (mut pss, mut private, mut anon, mut swap) = (0, 0_u64, 0, 0);
    for (name, bytes) in text.lines().filter_map(field) {
        match name {
            "Rss" => rss = Some(bytes),
            "Pss" => pss = bytes,
            "Private_Clean" | "Private_Dirty" => private = private.saturating_add(bytes),
            "Anonymous" => anon = bytes,
            "Swap" => swap = bytes,
            _ => {}
        }
    }
    Some(Usage {
        rss: rss?,
        pss: Some(pss),
        private: Some(private),
        anon: Some(anon),
        swap: Some(swap),
        footprint: None,
    })
}

/// 读 `status` 的内容里 `VmRSS` 那一行，字节。
pub fn parse_status_rss(text: &str) -> Option<u64> {
    text.lines()
        .filter_map(field)
        .find_map(|(name, bytes)| (name == "VmRSS").then_some(bytes))
}

/// 一行 `<名字>:<空白><数> kB`：名字和字节数。不是这个样子的没有。
fn field(line: &str) -> Option<(&str, u64)> {
    let (name, rest) = line.split_once(':')?;
    let kib: u64 = rest.trim().strip_suffix("kB")?.trim().parse().ok()?;
    Some((name.trim(), kib.saturating_mul(1024)))
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{Usage, parse_rollup, parse_status_rss};

    pub(super) fn usage() -> Option<Usage> {
        parse_rollup(&std::fs::read_to_string("/proc/self/smaps_rollup").ok()?)
    }

    pub(super) fn resident() -> Option<u64> {
        parse_status_rss(&std::fs::read_to_string("/proc/self/status").ok()?)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::Usage;

    pub(super) fn usage() -> Option<Usage> {
        let info = rusage()?;
        Some(Usage {
            rss: info.ri_resident_size,
            footprint: Some(info.ri_phys_footprint),
            ..Usage::default()
        })
    }

    pub(super) fn resident() -> Option<u64> {
        rusage().map(|info| info.ri_resident_size)
    }

    /// 自己的 `rusage_info_v2`：问不出的交回没有。两处 `unsafe` 各自写明为什么安全。
    #[allow(unsafe_code, reason = "调 macOS 的 proc_pid_rusage、读它填好的结构")]
    fn rusage() -> Option<libc::rusage_info_v2> {
        let pid = libc::c_int::try_from(std::process::id()).ok()?;
        let mut info = std::mem::MaybeUninit::<libc::rusage_info_v2>::zeroed();
        // SAFETY: `info` 是一块够大、对齐的 `rusage_info_v2`，照 `RUSAGE_INFO_V2` 这一版让内核填；`proc_pid_rusage` 只往这块里
        // 写，不留着指针。
        let done = unsafe {
            libc::proc_pid_rusage(
                pid,
                libc::RUSAGE_INFO_V2,
                info.as_mut_ptr().cast::<libc::rusage_info_t>(),
            )
        };
        // SAFETY: 交回 0 是填好了；全是整数的结构，先清零过，没填到的也是合法的值。
        (done == 0).then(|| unsafe { info.assume_init() })
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod imp {
    use super::Usage;

    pub(super) fn usage() -> Option<Usage> {
        None
    }

    pub(super) fn resident() -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests;

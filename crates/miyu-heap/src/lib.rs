//! 进程的堆（`internal/designs/23-性能预算.md` F3，施工 V-2 再补）：核心用系统的分配器；glibc 上调两个参数，空闲时把空着的还给
//! 系统。另交出堆的统计、进程这时占了多少内存，给核心自己记账（F4，施工 V-4，[`usage`]）；这台机器还剩多少可用内存，核心照它
//! 判内存紧不紧、要不要把闲着的会话卸出内存（施工 MEM-1 读法，MEM-2 改成卸会话，[`room`]）。
//!
//! glibc 释放的内存多半留在自己手里，不还给系统：核心空闲时模型目录读完剩下的、关掉的大会话放下的，进程的内存都不回落（V-2 中
//! 量过：一万条事件的会话关掉以后核心还占 48 MB，空闲时 27 MB）。macOS 的分配器也留着：放掉的块等着复用，系统内存紧了才
//! 还（施工 LW-18，2026-11-22 在一台 M1 上量的：核心空闲时堆里写过 7.3 MB，在用的只有 1.7 MB），所以空闲时同样叫它还一次。
//! 别的平台（Windows、musl）的分配器自己会还，这里什么都不做。

/// glibc 上 arena 最多几个。默认是核数的 8 倍，多开的只多攒碎片；读日志分块在几个线程里一起解（`miyu-store` 的 `MOST`，8 个），
/// 少于它就抢锁：限到 2 个时重开一万条事件的会话，读日志从约 10 ms 慢到 16 ms（施工 V-2 再补量过）。
pub const ARENAS: i32 = 8;

/// 多大的一块直接向系统要、放掉当场还：128 KiB（glibc 起初的默认）。写明了 glibc 就不再自己往上调：默认放掉一块大的以后把门槛
/// 抬到它那么大，之后每一轮拼的请求（几百 KB）都落进堆里，和长住的穿插着放，空出来的碎成小片、还不回去（施工 V-2 再补量过：
/// 大会话退下以后核心匿名内存比空闲时多 8.8 MB，写明以后多 3.9 MB；重开大会话、说一句以后 75 MB 降到 49 MB）。
pub const MMAP_FROM: i32 = 128 * 1024;

/// 一个 arena 的顶上那一块空着的（top）往上要的时候多要多少、往回还的时候留多少：0（glibc 默认 128 KiB）。施工 LW-25
/// 量过：核心的线程分配区放掉东西以后，每个 arena 顶上都留着这么一截不还，`malloc_trim` 只还主分配区的顶，线程分配区的顶它不管。
pub const TOP_PAD: i32 = 0;

/// 顶上空着的攒到多大才往回还：4 KiB（glibc 默认 128 KiB），和 [`TOP_PAD`] 一起，线程分配区放掉一块大的、并进顶以后当场还到
/// 只剩不满一页。施工 LW-25 量过（`internal/blueprint/heap.md`「怎么走」第 7 条）：核心连着一个头时堆和匿名内存少约 0.15 到
/// 0.35 MB，长会话说到第 30 轮少约 0.4 MB；量尺只有重开大会话后的头一次投影慢 0.15 到 0.5 ms，每一轮都走的几项没有变慢。
pub const TRIM_FROM: i32 = 4 * 1024;

/// 起来时、还没起别的线程时调一次：glibc 上 arena 限到 [`ARENAS`] 个，大块的门槛定在 [`MMAP_FROM`]，顶上不多留（[`TOP_PAD`]、
/// [`TRIM_FROM`]，施工 LW-25）。交回这几样都设成了没有；别的平台什么都不做，交回假。
///
/// Linux 上（不管是不是 glibc）还给这个进程关掉透明大页（`PR_SET_THP_DISABLE`，施工 YC-12）：系统设成 `always` 时，
/// `khugepaged` 把还有一页在用的 2 MB 一段合成大页，[`trim`] 还掉的空洞又被填回去（2026-11-10 压测量过：高压过后会话退下、
/// `trim` 以后 PSS 569 MB 降到 82 MB，接下来五分钟每分钟涨约 90 MB，回到约 476 MB 不再落，在用的堆只有 13 MB，
/// `AnonHugePages` 约等于整个匿名内存）。关没关上不在交回的里面，照 [`huge_pages_off`] 问。这个标记 `fork`、`exec` 都传下去：
/// 经助手跑的命令由助手换成命令之前恢复（`miyu-sandbox`），扩展、MCP 服务和不经助手的命令跟着关着，只是用不上大页。
pub fn tune() -> bool {
    huge::disable();
    imp::tune()
}

/// 这个进程的透明大页关着没有（`PR_GET_THP_DISABLE`，施工 YC-12）：给核心记一笔 [`tune`] 关上没有。别的平台交回假。
pub fn huge_pages_off() -> bool {
    huge::off()
}

/// 把堆里空着的还给系统：glibc 上调 `malloc_trim(0)`，macOS 上调 `malloc_zone_pressure_relief`（施工 LW-18）。交回还了
/// 没有；别的平台什么都不做，交回假。要走一遍堆，在阻塞线程里调。
pub fn trim() -> bool {
    imp::trim()
}

/// 堆这时的样子，字节（施工 V-4）：glibc 的 `mallinfo2`，各个 arena 加起来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// 各个 arena 从系统要来的（不算单独 mmap 的大块）。
    pub arenas: u64,
    /// 单独 mmap 的大块，放掉当场还。
    pub mmapped: u64,
    /// arena 里分出去、在用的。
    pub used: u64,
    /// arena 里空着的：碎片加顶上那一块，`trim` 还得回去的在这里面。
    pub free: u64,
}

/// 堆这时的统计：glibc 上照 `mallinfo2`；别的平台没有。要走一遍各个 arena，在阻塞线程里调。
pub fn stats() -> Option<Stats> {
    imp::stats()
}

pub mod launch;
pub mod room;
pub mod usage;

#[cfg(target_os = "linux")]
mod huge {
    /// 关掉这个进程的透明大页。关不上（3.15 以前的内核没有这一项）照旧起来，只是还用大页：[`off`] 如实交回假，核心记进日志。
    pub(super) fn disable() {
        rustix::thread::disable_transparent_huge_pages(true).unwrap_or_default();
    }

    pub(super) fn off() -> bool {
        rustix::thread::transparent_huge_pages_are_disabled().unwrap_or(false)
    }
}

#[cfg(not(target_os = "linux"))]
mod huge {
    pub(super) fn disable() {}

    pub(super) fn off() -> bool {
        false
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod imp {
    use std::os::raw::c_int;

    /// glibc 的 `M_ARENA_MAX`（`malloc.h`）。
    const M_ARENA_MAX: c_int = -8;
    /// glibc 的 `M_MMAP_THRESHOLD`（`malloc.h`）。
    const M_MMAP_THRESHOLD: c_int = -3;
    /// glibc 的 `M_TOP_PAD`（`malloc.h`）。
    const M_TOP_PAD: c_int = -2;
    /// glibc 的 `M_TRIM_THRESHOLD`（`malloc.h`）。
    const M_TRIM_THRESHOLD: c_int = -1;

    /// glibc 的 `struct mallinfo2`（`malloc.h`，2.33 起）：十格，都是 `size_t`，照这个先后。
    #[repr(C)]
    #[allow(dead_code, reason = "照 malloc.h 占位：只读其中四格")]
    struct Mallinfo2 {
        arena: usize,
        ordblks: usize,
        smblks: usize,
        hblks: usize,
        hblkhd: usize,
        usmblks: usize,
        fsmblks: usize,
        uordblks: usize,
        fordblks: usize,
        keepcost: usize,
    }

    // 声明 glibc 的三个函数：声明本身不调用，调用的三处各自写明为什么安全。
    #[allow(unsafe_code, reason = "声明 glibc 的 mallopt、malloc_trim、mallinfo2")]
    unsafe extern "C" {
        fn mallopt(param: c_int, value: c_int) -> c_int;
        fn malloc_trim(pad: usize) -> c_int;
        fn mallinfo2() -> Mallinfo2;
    }

    /// 一样一样设，前一样没设成的照样设后面的：交回的是都设成了没有。
    pub(super) fn tune() -> bool {
        [
            (M_ARENA_MAX, super::ARENAS),
            (M_MMAP_THRESHOLD, super::MMAP_FROM),
            (M_TOP_PAD, super::TOP_PAD),
            (M_TRIM_THRESHOLD, super::TRIM_FROM),
        ]
        .into_iter()
        .fold(true, |all, (param, value)| set(param, value) && all)
    }

    /// 设一个分配器参数，交回设成了没有。
    fn set(param: c_int, value: c_int) -> bool {
        // SAFETY: `mallopt` 只改 glibc 分配器自己的参数，不读写调的一方的内存；参数是 glibc 认得的常量和一个不小于 0 的数。
        #[allow(unsafe_code, reason = "调 glibc 的 mallopt，见上一行")]
        let done = unsafe { mallopt(param, value) };
        done == 1
    }

    pub(super) fn trim() -> bool {
        // SAFETY: `malloc_trim` 只把分配器手里空着的页还给系统，已经分出去的不动；glibc 自己加锁，哪个线程调都行。
        #[allow(unsafe_code, reason = "调 glibc 的 malloc_trim，见上一行")]
        let released = unsafe { malloc_trim(0) };
        released == 1
    }

    pub(super) fn stats() -> Option<super::Stats> {
        // SAFETY: `mallinfo2` 只读分配器自己的统计、照值交回一个结构，不碰调的一方的内存；glibc 自己加锁，哪个线程调都行。
        // 结构照 `malloc.h` 写（上面的 `Mallinfo2`）。
        #[allow(unsafe_code, reason = "调 glibc 的 mallinfo2，见上一行")]
        let info = unsafe { mallinfo2() };
        let bytes = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
        Some(super::Stats {
            arenas: bytes(info.arena),
            mmapped: bytes(info.hblkhd),
            used: bytes(info.uordblks),
            free: bytes(info.fordblks),
        })
    }
}

/// macOS（施工 LW-18）：系统的分配器把放掉的块留在手里等着复用，系统内存紧了才还（`vmmap` 的 MALLOC ZONE 那张表里
/// 「FRAG SIZE」那一栏）。`malloc_zone_pressure_relief` 就是内存紧了时系统替每个进程调的那一步，这里在空下来的时候自己调：
/// 只还空着的，已经分出去的不动。还回去的页不再算进这个进程的占用（`phys_footprint`，活动监视器「内存」那一列）。
#[cfg(target_os = "macos")]
mod imp {
    // 声明 libSystem 的一个函数（`malloc/malloc.h`，10.7 起）：`zone` 是空指针时每个分配区都还，`goal` 是 0 时能还多少还多少，
    // 交回还了多少字节。
    #[allow(unsafe_code, reason = "声明 libSystem 的 malloc_zone_pressure_relief")]
    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
    }

    pub(super) fn tune() -> bool {
        false
    }

    /// 交回还了没有。这个函数自己交回的字节数靠不住（实测放掉 32 MB 小块、占用落回去了，它交回 0），所以另看这个进程的
    /// 占用（`phys_footprint`）前后落了没有：两样里有一样说还了就算还了。
    pub(super) fn trim() -> bool {
        let footprint = || crate::usage::usage().and_then(|usage| usage.footprint);
        let before = footprint();
        // SAFETY: `malloc_zone_pressure_relief` 只把分配器手里空着的页、缓存着的空块还给系统，已经分出去的不动；参数是文档
        // 写明认的「所有分配区」（空指针）和「能还多少还多少」（0）。分配器自己加锁，哪个线程调都行。
        #[allow(unsafe_code, reason = "调 malloc_zone_pressure_relief，见上一行")]
        let released = unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
        released > 0
            || before
                .zip(footprint())
                .is_some_and(|(before, after)| after < before)
    }

    pub(super) fn stats() -> Option<super::Stats> {
        None
    }
}

/// 别的（Linux 的 musl、Windows、BSD）：分配器自己会还，什么都不做。
#[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "macos")))]
mod imp {
    pub(super) fn tune() -> bool {
        false
    }

    pub(super) fn trim() -> bool {
        false
    }

    pub(super) fn stats() -> Option<super::Stats> {
        None
    }
}

#[cfg(test)]
mod tests;

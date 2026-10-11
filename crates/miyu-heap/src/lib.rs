//! 进程的堆（`docs/designs/23-性能预算.md` F3，施工 V-2 再补）：核心用系统的分配器；glibc 上调两个参数，空闲时把空着的还给
//! 系统。另交出堆的统计、进程这时占了多少内存，给核心自己记账（F4，施工 V-4，[`usage`]）。
//!
//! glibc 释放的内存多半留在自己手里，不还给系统：核心空闲时模型目录读完剩下的、关掉的大会话放下的，进程的内存都不回落（V-2 中
//! 量过：一万条事件的会话关掉以后核心还占 48 MB，空闲时 27 MB）。别的平台（macOS、Windows、musl）的分配器自己会还，这里
//! 什么都不做。

/// glibc 上 arena 最多几个。默认是核数的 8 倍，多开的只多攒碎片；读日志分块在几个线程里一起解（`miyu-store` 的 `MOST`，8 个），
/// 少于它就抢锁：限到 2 个时重开一万条事件的会话，读日志从约 10 ms 慢到 16 ms（施工 V-2 再补量过）。
pub const ARENAS: i32 = 8;

/// 多大的一块直接向系统要、放掉当场还：128 KiB（glibc 起初的默认）。写明了 glibc 就不再自己往上调：默认放掉一块大的以后把门槛
/// 抬到它那么大，之后每一轮拼的请求（几百 KB）都落进堆里，和长住的穿插着放，空出来的碎成小片、还不回去（施工 V-2 再补量过：
/// 大会话退下以后核心匿名内存比空闲时多 8.8 MB，写明以后多 3.9 MB；重开大会话、说一句以后 75 MB 降到 49 MB）。
pub const MMAP_FROM: i32 = 128 * 1024;

/// 起来时、还没起别的线程时调一次：glibc 上 arena 限到 [`ARENAS`] 个，大块的门槛定在 [`MMAP_FROM`]。交回两样都设成了没有；
/// 别的平台什么都不做，交回假。
pub fn tune() -> bool {
    imp::tune(ARENAS, MMAP_FROM)
}

/// 把堆里空着的还给系统：glibc 上调 `malloc_trim(0)`。交回还了没有；别的平台什么都不做，交回假。要走一遍堆，在阻塞线程里调。
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

pub mod usage;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod imp {
    use std::os::raw::c_int;

    /// glibc 的 `M_ARENA_MAX`（`malloc.h`）。
    const M_ARENA_MAX: c_int = -8;
    /// glibc 的 `M_MMAP_THRESHOLD`（`malloc.h`）。
    const M_MMAP_THRESHOLD: c_int = -3;

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

    pub(super) fn tune(arenas: i32, mmap_from: i32) -> bool {
        set(M_ARENA_MAX, arenas) && set(M_MMAP_THRESHOLD, mmap_from)
    }

    /// 设一个分配器参数，交回设成了没有。
    fn set(param: c_int, value: c_int) -> bool {
        // SAFETY: `mallopt` 只改 glibc 分配器自己的参数，不读写调的一方的内存；参数是 glibc 认得的常量和一个正数。
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

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
mod imp {
    pub(super) fn tune(_arenas: i32, _mmap_from: i32) -> bool {
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

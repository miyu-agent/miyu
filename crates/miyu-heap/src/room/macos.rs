//! macOS 上还剩多少内存（施工 MEM-1）：`kern.memorystatus_level` 是内核自己算的可用百分比（活动监视器「内存压力」照的那一个），
//! 总量照 `hw.memsize`。哪一个问不出的交回没有。

/// 照 [`super::macos_of`] 算：两个都问得出才有。
pub(super) fn room() -> Option<super::Room> {
    let level: libc::c_int = sysctl(c"kern.memorystatus_level")?;
    let total: u64 = sysctl(c"hw.memsize")?;
    super::macos_of(u64::try_from(level).ok()?, total)
}

/// 问一个整数的 sysctl：写回的字节数和 `T` 一样大才算问到了。
#[allow(unsafe_code, reason = "调 macOS 的 sysctlbyname，见下面的 SAFETY")]
fn sysctl<T: Copy + Default>(name: &std::ffi::CStr) -> Option<T> {
    let mut value = T::default();
    let mut size = std::mem::size_of::<T>();
    // SAFETY: `name` 是以 0 结尾的字；`value` 是一块 `size` 字节、对齐的 `T`，内核最多往里写 `size` 字节、把写了多少写回
    // `size`；不改设置（新值是空指针、长度 0）；不留着指针。`T` 只用整数，哪种字节都是合法的值。
    let done = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&raw mut value).cast::<libc::c_void>(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (done == 0 && size == std::mem::size_of::<T>()).then_some(value)
}

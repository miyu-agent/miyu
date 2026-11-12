//! Windows 上还剩多少内存（施工 MEM-1）：`GlobalMemoryStatusEx` 的可用物理内存、物理内存总量。问不出的交回没有。

use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

/// 问一次系统。
#[allow(
    unsafe_code,
    reason = "调 Windows 的 GlobalMemoryStatusEx，见下面的 SAFETY"
)]
pub(super) fn room() -> Option<super::Room> {
    // SAFETY: `MEMORYSTATUSEX` 全是整数，清零的是合法的值。
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).ok()?;
    // SAFETY: `status` 是一块对齐的 `MEMORYSTATUSEX`，`dwLength` 照接口的要求写好了它的大小；系统只往里写，不留着指针。
    let done = unsafe { GlobalMemoryStatusEx(&raw mut status) };
    if done == 0 {
        return None;
    }
    super::room_of(status.ullAvailPhys, status.ullTotalPhys)
}

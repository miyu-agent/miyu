//! 这台机器的固定盘（施工 5-9 改上）：设计 11 A4「读整盘放开」，每个固定盘的盘根进只读路径（MXC 照路径登记，不往下走，给多大
//! 都不多花，2026-10-21 量过）。可移动盘、网络盘、光驱不给。

#![allow(unsafe_code, reason = "调 Windows 的系统接口（施工 5-9 改上）")]

use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};

/// 固定盘的类型（`DRIVE_FIXED`）。
const FIXED: u32 = 3;

/// 固定盘的盘根，`C:\` 这样写。
pub(super) fn fixed() -> Vec<String> {
    // SAFETY: 没有参数，只交回一个位图。
    let mask = unsafe { GetLogicalDrives() };
    (0..26_u8)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| format!("{}:\\", char::from(b'A' + bit)))
        .filter(|root| {
            let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
            // SAFETY: `wide` 以 0 结尾，调用期间一直活着。
            unsafe { GetDriveTypeW(wide.as_ptr()) == FIXED }
        })
        .collect()
}

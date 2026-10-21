//! 有没有桌面会话（施工 5-9 改上）：进程的窗口站是 `WinSta0` 的是登录的桌面；SSH、服务里的是别的窗口站，容器里要窗口的程序
//! 起不来（0xC0000142，2026-10-20 虚拟机上量过）。

#![allow(unsafe_code, reason = "调 Windows 的系统接口（施工 5-9 改上）")]

use windows_sys::Win32::System::StationsAndDesktops::{
    GetProcessWindowStation, GetUserObjectInformationW, UOI_NAME,
};

/// 交互的窗口站的名字。
const INTERACTIVE: &str = "WinSta0";

/// 这个进程在登录的桌面会话里。查不出来的当不在。
pub(super) fn interactive() -> bool {
    // SAFETY: 没有参数；交回的窗口站的句柄归系统，不用关。
    let station = unsafe { GetProcessWindowStation() };
    if station.is_null() {
        return false;
    }
    let mut name = [0_u16; 64];
    let mut needed = 0_u32;
    let size = u32::try_from(std::mem::size_of_val(&name)).unwrap_or(0);
    // SAFETY: `name` 是 `size` 个字节的缓冲区，系统最多写这么多；`needed` 是有效的 u32。
    let ok = unsafe {
        GetUserObjectInformationW(
            station,
            UOI_NAME,
            name.as_mut_ptr().cast(),
            size,
            &mut needed,
        )
    };
    if ok == 0 {
        return false;
    }
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    String::from_utf16_lossy(&name[..len]).eq_ignore_ascii_case(INTERACTIVE)
}

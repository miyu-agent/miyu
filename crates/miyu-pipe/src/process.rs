//! 问一个进程还在不在（施工 WC-1）：脚本调工具的通道在临时目录下一个核心进程一个 `miyu-call-<进程号>`，核心第一次用时删掉
//! 进程已经不在了的那些（`miyu-basesystem` 的 `shell/channel/leftovers.rs`）。Unix 上 `kill(pid, 0)` 一句就问得出，Windows
//! 上要打开那个进程看它的退出码，要调系统接口，放在这里。
//!
//! 问一个进程什么时候起的（施工 XG-1补2）：认扩展的子孙时照父进程链往上找，Windows 不过继，链上的父进程号可能被后起的进程
//! 重用，要比创建时间才认得出来（`miyu-call` 的 `kin.rs`）。

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows_sys::Win32::Foundation::{ERROR_INVALID_PARAMETER, FILETIME, STILL_ACTIVE};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// 进程 `pid` 还在跑没有。系统说没有这个进程的（`ERROR_INVALID_PARAMETER`）、打得开但已经退了的（还有人拿着它的句柄）是
/// 不在了；打不开的别的情形（别的用户的、受保护的：拒绝访问）、问不出退出码的都当还在：拿不准的不删。进程号被别的进程
/// 重用了的也算在，下次再看。
pub fn process_running(pid: u32) -> bool {
    // SAFETY: 只要查询信息的权限、不继承；拿到的句柄下面交给 `OwnedHandle`，丢掉时关。
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        let gone =
            io::Error::last_os_error().raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32);
        return !gone;
    }
    // SAFETY: `raw` 是刚打开的进程句柄，只在这里接管，丢掉时关一次。
    let process = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut code = 0u32;
    // SAFETY: 句柄在调用期间有效；`code` 是给系统写的出参。
    if unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) } == 0 {
        return true;
    }
    // 退出码正好是 259 的已经退了的进程也当还在：只是晚一次删。
    code == STILL_ACTIVE as u32
}

/// 进程 `pid` 什么时候起的：`GetProcessTimes` 的创建时间，1601 年起的 100 纳秒数，只拿来和别的进程比先后。打不开的（不在了、
/// 别的用户的、受保护的）、问不出的没有。已经退了、还有人拿着句柄的照样问得出：编号还没放出去，不会是重用了的。
pub fn process_created(pid: u32) -> Option<u64> {
    // SAFETY: 只要查询信息的权限、不继承；拿到的句柄下面交给 `OwnedHandle`，丢掉时关。
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        return None;
    }
    // SAFETY: `raw` 是刚打开的进程句柄，只在这里接管，丢掉时关一次。
    let process = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: 句柄在调用期间有效；四个 `FILETIME` 都是给系统写的出参，各自是一块独立的、大小正好的内存。
    let got = unsafe {
        GetProcessTimes(
            process.as_raw_handle(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    if got == 0 {
        return None;
    }
    Some(u64::from(created.dwHighDateTime) << 32 | u64::from(created.dwLowDateTime))
}

#[cfg(test)]
mod tests;

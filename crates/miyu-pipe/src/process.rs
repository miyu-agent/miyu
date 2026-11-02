//! 问一个进程还在不在（施工 WC-1）：脚本调工具的通道在临时目录下一个核心进程一个 `miyu-call-<进程号>`，核心第一次用时删掉
//! 进程已经不在了的那些（`miyu-basesystem` 的 `shell/channel/leftovers.rs`）。Unix 上 `kill(pid, 0)` 一句就问得出，Windows
//! 上要打开那个进程看它的退出码，要调系统接口，放在这里。

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows_sys::Win32::Foundation::{ERROR_INVALID_PARAMETER, STILL_ACTIVE};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
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

#[cfg(test)]
mod tests;

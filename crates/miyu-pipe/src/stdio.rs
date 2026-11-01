//! 标准句柄不让子进程继承（施工 CS-2）：Windows 上标准库拉起子进程时 `bInheritHandles` 是真的，本进程所有标成能继承的句柄
//! 都跟着进去，不只是给子进程的那三个。`miyu ask` 的标准输入是重定向来的文件时，它拉起的核心就一直开着那个文件，`ask`
//! 早退了文件还删不掉（2026-11-02 桌面端在虚拟机上撞到三次）。拉起常驻的子进程之前把本进程的三个标准句柄标成不能继承：
//! 子进程要用的标准输入输出由标准库另复制一份能继承的交给它，不受影响。

use windows_sys::Win32::Foundation::{
    HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

/// 本进程的标准输入、输出、错误都标成不能继承。没有的（没接控制台、句柄无效）跳过；改不成的不管：最坏是照旧被继承。
pub fn uninherit_std_handles() {
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: `GetStdHandle` 只读本进程的标准句柄表，参数是这三个常量之一，不碰调用方的内存。
        let handle = unsafe { GetStdHandle(which) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            continue;
        }
        // SAFETY: `handle` 是本进程的标准句柄，上面判过不是空的、不是无效值；只清它的继承标记，不关它、不改别的标记。
        unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
    }
}

#[cfg(test)]
mod tests;

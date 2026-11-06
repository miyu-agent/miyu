//! 问进程的父进程（施工 XG-1 下，设计 `63-扩展工具的效果和声明.md` 第六节第 2 条）：脚本调工具的通道认「连上来的是不是这个
//! 扩展进程的子孙」，照父进程一层层往上找（`miyu-call` 的 `kin.rs`）。Linux 读 `/proc` 就问得出，不用这里；macOS 要
//! `proc_pidinfo`，Windows 要拍一张进程快照，都要调系统接口，放在这里。

/// 进程 `pid` 的父进程（macOS，`proc_pidinfo` 的 `PROC_PIDTBSDINFO`）：问不出的（不在了、别的用户的）没有。父进程先走了的，
/// 系统把它过继给 `launchd`（1 号）：问出来的父进程总是还在的那一个，进程号不会是被重用了的旧编号。
#[cfg(target_os = "macos")]
pub fn parent_process(pid: u32) -> Option<u32> {
    let pid = libc::c_int::try_from(pid).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: `info` 是一块大小正好是 `proc_bsdinfo` 的、全零的内存，`size` 照实说它多大；系统最多写这么多字节，写满了才交回
    // `size`，没写满的下面不读。
    let wrote = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if wrote != size {
        return None;
    }
    // SAFETY: 上面系统写满了整块；全零开头也是这个只有整数、字符数组的结构的合法值。
    let info = unsafe { info.assume_init() };
    Some(info.pbi_ppid)
}

/// 这一刻每个进程的父进程（Windows，`CreateToolhelp32Snapshot`）：进程号到父进程号。Windows 不过继：父进程走了以后它的编号
/// 可能被别的进程重用，查的一方要知道「父进程号」可能指着一个后来才起的进程：照 [`crate::process_created`] 比先后
/// （施工 XG-1补2）。
///
/// # Errors
///
/// 快照拍不成、读不出第一项：交回系统的原话。
#[cfg(windows)]
pub fn process_parents() -> std::io::Result<std::collections::HashMap<u32, u32>> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    // SAFETY: 只拍进程的快照，不带别的；拿到的句柄下面交给 `OwnedHandle`，丢掉时关。
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `raw` 是刚拍的快照句柄，只在这里接管，丢掉时关一次。
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>())
            .map_err(std::io::Error::other)?,
        ..PROCESSENTRY32W::default()
    };
    let mut parents = std::collections::HashMap::new();
    // SAFETY: 句柄在调用期间有效；`entry` 是给系统写的出参，`dwSize` 照实填了它多大。
    if unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    loop {
        parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
        // SAFETY: 同上。读完了（`ERROR_NO_MORE_FILES`）交回 0。
        if unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) } == 0 {
            return Ok(parents);
        }
    }
}

#[cfg(test)]
mod tests;

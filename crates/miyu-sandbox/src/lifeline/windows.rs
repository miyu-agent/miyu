//! Windows 上的生命线（`lifeline.rs`）：核心自己进一个「关了就全结束」的作业对象。
//!
//! 这是放开 `unsafe` 的几个模块之一（施工 7-8）：每个 `unsafe` 块前面写着为什么安全。

#![allow(unsafe_code, reason = "调 Windows 的系统接口（施工 7-8）")]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr;
use std::sync::{Arc, OnceLock};

use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// 建一个作业对象，设成最后一个句柄关了就结束里面的全部，把这个进程放进去。句柄一直拿到进程结束：进程没了，系统关掉它，
/// 作业里的都结束。只做一次，再调直接交回上一次的结果。
pub(super) fn bind_children() -> io::Result<()> {
    static JOB: OnceLock<Result<OwnedHandle, String>> = OnceLock::new();
    JOB.get_or_init(|| bind().map_err(|error| error.to_string()))
        .as_ref()
        .map(|_| ())
        .map_err(|error| io::Error::other(error.clone()))
}

fn bind() -> io::Result<OwnedHandle> {
    // SAFETY: 没有安全属性、没有名字，传空；交回的是新建的句柄或者空。
    let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` 是刚建成的作业对象的句柄，只在这里接管，丢掉时关一次。
    let job = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let size = u32::try_from(std::mem::size_of_val(&limits)).map_err(io::Error::other)?;
    // SAFETY: `limits` 在调用期间有效，大小照实给；系统只读它。
    let set = unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            size,
        )
    };
    if set == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `GetCurrentProcess` 交回的是这个进程的伪句柄，不用关；作业的句柄上面拿着，有效。
    let assigned = unsafe { AssignProcessToJobObject(job.as_raw_handle(), GetCurrentProcess()) };
    if assigned == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}

/// 一条命令的作业对象（施工 M-3 下，设计 `39-脚本调工具.md` 第五节第 3 条）：命令起来以后马上放进去，它再起的都在里面
/// （作业对象照继承，套在核心那个里面，Windows 8 起能嵌套）。脚本调工具的通道照它认连上来的进程：在里面的才是这条命令的。
/// 只用来认人：不设限制，关了也不结束里面的（整组杀照旧用 `taskkill /T`，2026-10-20 照推荐定）。
#[derive(Debug, Clone)]
pub struct Job(Arc<OwnedHandle>);

impl Job {
    /// 建一个，把 `child` 放进去。`child` 起来到放进去之间它起的不在里面：PowerShell 起来要上百毫秒，碰不到。
    ///
    /// # Errors
    ///
    /// 建不成、放不进去（例如它所在的作业不许再套）：交回系统的原话。
    pub fn for_child(child: &std::process::Child) -> io::Result<Job> {
        // SAFETY: 没有安全属性、没有名字，传空；交回的是新建的句柄或者空。
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` 是刚建成的作业对象的句柄，只在这里接管，丢掉时关一次。
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        // SAFETY: 两个句柄在调用期间都有效：作业的上面拿着，进程的由 `child` 拿着。
        let assigned =
            unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Job(Arc::new(job)))
    }

    /// 进程 `pid` 在不在里面。打不开那个进程的（已经走了、别人的）算不在。
    pub fn contains(&self, pid: u32) -> bool {
        // SAFETY: 只要查询信息的权限；拿到的句柄下面接管，丢掉时关。
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if raw.is_null() {
            return false;
        }
        // SAFETY: `raw` 是刚打开的进程句柄，只在这里接管。
        let process = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut inside = 0;
        // SAFETY: 两个句柄在调用期间都有效；`inside` 是给系统写的出参。
        let asked =
            unsafe { IsProcessInJob(process.as_raw_handle(), self.0.as_raw_handle(), &mut inside) };
        asked != 0 && inside != 0
    }
}

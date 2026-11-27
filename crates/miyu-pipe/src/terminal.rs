//! 这个进程有没有控制终端（Unix，施工 CS-11）：照 `open("/dev/tty", O_RDWR | O_NOCTTY | O_CLOEXEC)` 问，打开了就是有，打不开
//! （没有控制终端是 `ENXIO`，别的原因一样算没有）就是没有。人在终端前台直接跑 `miyu core` 的有；头拉起的核心（另开了会话，
//! 施工 CS-10）、systemd、launchd 起的没有。`O_NOCTTY` 照 POSIX 要标志位，标准库没有，所以和别的系统接口放在一起。

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;

/// 这个进程有没有控制终端（见这一页开头）。
pub fn controlling_terminal() -> bool {
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open("/dev/tty")
        .is_ok()
}

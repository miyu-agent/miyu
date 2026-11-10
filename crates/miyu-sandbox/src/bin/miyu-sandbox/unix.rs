//! Unix 上换成那条命令（`exec`）：同一个进程，工作目录、环境变量、标准输入输出、进程组都照旧。Linux、macOS 收紧
//! 以后都走这里。

use std::ffi::{OsStr, OsString};
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

/// 换成 `program`：换成了就不回来，换不成才交回退出码。
///
/// Linux 上换之前先恢复透明大页（施工 YC-12）：核心给自己关了（`miyu-heap` 的 `tune`），这个标记 `fork`、`exec` 都传下来；
/// 模型跑的命令（编译器这些）照系统的默认用大页。
pub(crate) fn exec(program: &OsStr, args: &[OsString]) -> ExitCode {
    #[cfg(target_os = "linux")]
    {
        // 恢复不了（3.15 以前的内核没有这一项）照旧换成命令，只是还关着大页：不值得为它拦下命令；助手自己什么都不多印。
        rustix::thread::disable_transparent_huge_pages(false).unwrap_or_default();
    }
    let error = Command::new(program).args(args).exec();
    crate::cannot_run(program, &error)
}

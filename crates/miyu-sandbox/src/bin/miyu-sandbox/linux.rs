//! Linux 上的收紧（`docs/blueprint/sandbox/linux.md`）：只用 Landlock（施工 5-2 起；5-3 照 DeepSeek 的 dsh 改成整盘能
//! 读、只管写）。不要命名空间、不要 AppArmor（2026-09-29 项目主人定）。
//!
//! Landlock 只能放行，不能在放行的范围里再挖掉一块：「除了藏起来的都能读」是一级级放行做出来的（[`reads`]）；藏起来的
//! 落在能写的里面，写也照这样一级级绕开（施工 5-15，原来拒绝执行）：路上的那几层目录本身不能新建、删除、改名。

mod landlock;
mod reads;

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use miyu_sandbox::Spec;

/// 这台机器上能用上的收紧手段：`probe` 印的。
pub(crate) fn mechanisms() -> Vec<String> {
    match landlock::available() {
        Ok(()) => vec!["landlock".to_string()],
        Err(_) => Vec::new(),
    }
}

/// 照规格 `spec` 收紧，再换成那条命令。收不住的说原话，退出 125，不跑命令。
pub(crate) fn run(spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    if let Err(why) = landlock::confine(spec) {
        return crate::fail(&format!("cannot confine: {why}"));
    }
    crate::unix::exec(program, args)
}

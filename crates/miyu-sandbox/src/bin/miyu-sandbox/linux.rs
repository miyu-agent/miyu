//! Linux 上的收紧（`docs/blueprint/sandbox/linux.md`）：只用 Landlock（施工 5-2 起；5-3 照 DeepSeek 的 dsh 改成整盘能
//! 读、只管写）。不要命名空间、不要 AppArmor（2026-09-29 项目主人定）。
//!
//! Landlock 只能放行，不能在放行的范围里再挖掉一块：「除了藏起来的都能读」是一级级放行做出来的（[`reads`]）；藏起来的
//! 落在能写的里面，写也照这样一级级绕开（施工 5-15，原来拒绝执行）：路上的那几层目录本身不能新建、删除、改名。有能用的
//! bubblewrap 时规格写着 `mount`，用挂载盖住（[`bwrap`]，施工 5-16），不冻住那几层。

mod bwrap;
mod landlock;
mod reads;

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use miyu_sandbox::Spec;

/// 这台机器上能用上的收紧手段：`probe` 印的。有能用的 bubblewrap 的多一个 `bwrap`（施工 5-16）：它要在里面再用 Landlock，
/// 所以只在 Landlock 也能用时报。
pub(crate) fn mechanisms() -> Vec<String> {
    match landlock::available() {
        Ok(()) if bwrap::usable() => vec!["landlock".to_string(), "bwrap".to_string()],
        Ok(()) => vec!["landlock".to_string()],
        Err(_) => Vec::new(),
    }
}

/// 照规格 `spec` 收紧，再换成那条命令。收不住的说原话，退出 125，不跑命令。
pub(crate) fn run(spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    // 规格要挂载的换成 bubblewrap，挂好以后在里面再跑一次自己（施工 5-16）。
    if spec.mount {
        return bwrap::exec(spec, program, args);
    }
    if let Err(why) = landlock::confine(spec) {
        return crate::fail(&format!("cannot confine: {why}"));
    }
    crate::unix::exec(program, args)
}

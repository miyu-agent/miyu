//! Windows 上的收紧（施工 5-9 改上，`docs/blueprint/sandbox/windows.md`）：照规格拼一个 MXC 的请求（`mxc-sdk`，ProcessContainer），
//! 起那条命令、把标准输入、输出、错误原样转过去，照它的退出码退出。Windows 换不了进程，助手一直在：助手被杀，容器里的整棵
//! 进程树跟着停（MXC 的作业对象，2026-10-21 虚拟机上量过）。
//!
//! 只认基础容器那一档（`base-container`，不用改访问控制）：MXC 起不来基础容器时会退到给目录加访问控制项的那几档，助手被杀时
//! 那些项撤不回去，留在你的盘上（2026-10-21 定）。没有桌面会话（SSH、服务里）的照样报手段，每条命令照实拒绝（125），说为什么、
//! 怎么办：要窗口的程序（PowerShell、git）在容器里起不来（0xC0000142），不放宽、不转去问人（2026-10-20 项目主人定「拒绝并说
//! 清楚」）。拒绝的那一句就是这条命令的输出，她和你都看得到，不另加给她看的字。

mod desktop;
mod drives;
mod pump;
mod request;
mod resolve;

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use mxc_sdk::v1;

use miyu_sandbox::Spec;

/// 能用的那一档：MXC 探测报的。
const TIER: &str = "base-container";

/// 探测时报的手段名。
const MECHANISM: &str = "processcontainer";

/// 没有桌面会话时说的。
const NO_DESKTOP: &str = "cannot confine: no desktop session, programs in the sandbox cannot start here; use Miyu from the desktop, or turn the sandbox off";

/// 这台机器上能用上的收紧手段：`probe` 印的。MXC 探得出基础容器的报 `processcontainer`；有没有桌面会话是跑的时候的事。
pub(crate) fn mechanisms() -> Vec<String> {
    match container() {
        Ok(()) => vec![MECHANISM.to_string()],
        Err(_) => Vec::new(),
    }
}

/// 这一刻能不能用：不能的说为什么（英文一句，同别的平台 `cannot confine: …`）。
fn usable() -> Result<(), String> {
    if !desktop::interactive() {
        return Err(NO_DESKTOP.to_string());
    }
    container()
}

/// MXC 起得了基础容器。
fn container() -> Result<(), String> {
    let probed = v1::probe(None).map_err(|error| format!("cannot confine: {error}"))?;
    if probed.tier == Some(TIER) && probed.needs_dacl_augmentation != Some(true) {
        Ok(())
    } else {
        Err(format!(
            "cannot confine: this Windows has no base container (MXC tier {:?})",
            probed.tier
        ))
    }
}

/// 测试版的助手收不住时：起那条命令、等它，照它的退出码退出（5-9 以前的样子）。
#[cfg(feature = "testkit")]
fn passthrough(program: &OsStr, args: &[OsString]) -> ExitCode {
    match std::process::Command::new(program).args(args).status() {
        Ok(status) => std::process::exit(
            status
                .code()
                .unwrap_or(i32::from(miyu_sandbox::EXIT_HELPER)),
        ),
        Err(error) => crate::cannot_run(program, &error),
    }
}

/// 照规格 `spec` 收紧，起那条命令、等它：退出码照原样交回去。收不住的说一句、退出 125，不跑命令。
pub(crate) fn run(spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    // 先认程序：找不到的 127、不是程序的 126，同别的平台（MXC 起不来的分不出是哪一种）。
    if let Err(error) = resolve::program(program) {
        return crate::cannot_run(program, &error);
    }
    if let Err(why) = usable() {
        // 测试版的助手：收不住时照 5-9 以前直接起（`miyu_sandbox::testkit::PASSTHROUGH`），正式的没有这一段。
        #[cfg(feature = "testkit")]
        if std::env::var_os(miyu_sandbox::testkit::PASSTHROUGH).is_some() {
            return passthrough(program, args);
        }
        return crate::fail(&why);
    }
    let request = match request::of(spec, program, args, drives::fixed()) {
        Ok(request) => request,
        Err(why) => return crate::fail(&format!("cannot confine: {why}")),
    };
    match v1::spawn(request, v1::SpawnOptions::default()) {
        Ok(process) => pump::through(process),
        Err(error) => crate::fail(&format!("cannot confine: {error}")),
    }
}

//! 起来时找沙盒的助手、探一次（`docs/blueprint/sandbox.md`「怎么走」第 1、2 条，施工 5-1）：记一行日志，探到的结果交给
//! 协议端点：握手时报给头，能用的把助手交给会话（施工 5-4 上、下）。再算出沙盒的缓存放在哪（施工 5-4 下）。起不起得来
//! 不看这两样。助手和系统没换的照上一次探到的用，后台再探一遍（施工 V-5 上，[`remembered`]）。

mod remembered;

use std::path::{Path, PathBuf};

use miyu_sandbox::{Availability, Unusable};
use miyu_store::root::RootError;
use std::time::Duration;

use crate::TARGET;

/// 最多等助手多久。
const WAIT: Duration = Duration::from_secs(5);

/// 主程序的真实位置是 `exe`（环境快照里的，顺着链接找到的本体）：找旁边的助手、探一次、记一行，`INFO sandbox`
/// 或者 `WARN sandbox unavailable`。不知道主程序在哪的，也当找不到。探到了手段的能用，带着助手；手段是空的、找不到、
/// 探不成的用不了，带着原因。
///
/// 给了状态目录 `state` 的（施工 V-5 上）：助手和系统都没换、上一次探成了的，照记下的用，`INFO` 那一行多一格
/// `remembered=true`，另起一个线程再探一遍、照新的记下（下一次起来用新的，这一次不改）；没记过、对不上的照常探，探成了
/// 记下。探不成的不记：下一次照常探。
pub(crate) fn probe(exe: Option<&Path>, state: Option<&Path>) -> Availability {
    let Some(helper) = exe.and_then(miyu_sandbox::locate) else {
        tracing::warn!(target: TARGET, reason = "helper not found", "sandbox unavailable");
        return Availability::Unusable(Unusable::HelperMissing);
    };
    let key = state.and(remembered::key(&helper));
    if let (Some(state), Some(key)) = (state, &key)
        && let Some(probe) = remembered::recall(state, key)
    {
        log(&helper, &probe, true);
        let (again, state, key) = (helper.clone(), state.to_path_buf(), key.clone());
        std::thread::spawn(move || {
            if let Ok(probe) = miyu_sandbox::probe(&again, WAIT) {
                remembered::remember(&state, &key, &probe);
            }
        });
        return availability(helper, &probe);
    }
    match miyu_sandbox::probe(&helper, WAIT) {
        Ok(probe) => {
            log(&helper, &probe, false);
            if let (Some(state), Some(key)) = (state, &key) {
                remembered::remember(state, key, &probe);
            }
            availability(helper, &probe)
        }
        Err(error) => {
            tracing::warn!(target: TARGET, reason = %error, "sandbox unavailable");
            Availability::Unusable(Unusable::HelperFailed)
        }
    }
}

/// 探到的记一行 `INFO sandbox`；照上一次记下的用的多一格 `remembered=true`。
fn log(helper: &Path, probe: &miyu_sandbox::Probe, remembered: bool) {
    if remembered {
        tracing::info!(
            target: TARGET,
            helper = %helper.display(),
            platform = probe.platform.name(),
            mechanisms = %probe.mechanisms_text(),
            remembered = true,
            "sandbox"
        );
    } else {
        tracing::info!(
            target: TARGET,
            helper = %helper.display(),
            platform = probe.platform.name(),
            mechanisms = %probe.mechanisms_text(),
            "sandbox"
        );
    }
}

/// 探到了手段的能用，带着助手；手段是空的用不了。探成了的助手（构建 id 对得上）不管有没有手段，命令组里看门的都照它起
/// （施工 V-48，[`miyu_sandbox::lifeline::use_helper`]）。关起来的会话的 Unix 套接字照探到的设（施工 MB-1，[`miyu_sandbox::set_confines_sockets`]）：
/// 沙盒能用、套接字却管不住的（内核的 Landlock 太老、又没有 seccomp），关起来的场所会话不跑命令，记一行
/// `WARN sandbox cannot confine unix sockets`，好查群里为什么跑不了。
fn availability(helper: PathBuf, probe: &miyu_sandbox::Probe) -> Availability {
    miyu_sandbox::lifeline::use_helper(&helper);
    let sockets = miyu_sandbox::sockets_confined_by(&probe.mechanisms);
    miyu_sandbox::set_confines_sockets(sockets);
    if !sockets && !probe.mechanisms.is_empty() {
        tracing::warn!(
            target: TARGET,
            mechanisms = %probe.mechanisms_text(),
            "sandbox cannot confine unix sockets: members and venue sessions run no commands"
        );
    }
    if probe.mechanisms.is_empty() {
        Availability::Unusable(Unusable::NoMechanism)
    } else {
        Availability::Usable(helper)
    }
}

/// 算不出缓存目录的原因，运行日志里一律英文。
pub(crate) fn why(error: &RootError) -> &'static str {
    match error {
        RootError::NoHome => "no home directory",
        RootError::NoLocalAppData => "no LOCALAPPDATA",
        RootError::RelativeMiyuHome(_) => "relative MIYU_HOME",
    }
}

#[cfg(test)]
mod tests;

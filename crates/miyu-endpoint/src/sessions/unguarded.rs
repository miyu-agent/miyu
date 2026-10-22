//! 开会话那一句（施工 5-14 改，2026-10-22 项目主人定，主会话定形状）：Linux 上这一轮只用 Landlock（没有能用的 bubblewrap，
//! 或者设了弱沙盒），工作目录又包住数据根的，沙盒照 dsh 不在能写的里面挖洞，护不住 Miyu 自己的数据。`session.create`、
//! `session.send`（换了目录的那一次）、`session.set_workspace` 的回应多一格 `unguarded`：`no_bwrap` 或 `weak`，头说一次。

use std::path::Path;

use miyu_config::Values;
use miyu_session::settings::{SandboxSettings, WEAK};

use crate::Core;

/// 工作目录 `cwd`（头报的写法）这一轮护不护得住数据根：护不住的交回为什么（`no_bwrap`、`weak`）。别的平台（Seatbelt 藏得住、
/// Windows 照旧太宽）、有挂载又不是弱沙盒的、这台机器的沙盒用不了的（命令都要问人）、工作目录不包住数据根的（数据根在能写的
/// 以外照旧藏）、太宽退回了默认工作区的（根目录、数据根本身）没有。`values` 是这一轮的配置，看沙盒强度。
pub(crate) fn unguarded(core: &Core, cwd: &str, values: &Values) -> Option<&'static str> {
    if !cfg!(target_os = "linux") || core.sandbox.helper().is_none() {
        return None;
    }
    let home = crate::workspace::home(core);
    let real = miyu_fs::resolve(Path::new("/"), home.as_deref(), cwd).ok()?;
    let data_root =
        std::fs::canonicalize(core.root.path()).unwrap_or_else(|_| core.root.path().to_path_buf());
    if !data_root.starts_with(&real) || real == data_root || real.parent().is_none() {
        return None;
    }
    reason(
        miyu_sandbox::mounts(),
        &SandboxSettings::from(values).strength,
    )
}

/// 同 [`unguarded`]，这一轮的配置照工作目录 `cwd` 合（带上信任着的项目配置）：说话换了目录、换工作区的回应用它。
pub(crate) fn unguarded_at(core: &Core, cwd: &str) -> Option<&'static str> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    unguarded(core, cwd, &core.config().with_project(cwd).0.values())
}

/// 照这台机器能不能用挂载 `mounts`、这一轮的沙盒强度 `strength` 说为什么护不住：没有能用的 bubblewrap 的是 `no_bwrap`，有但设了
/// 弱沙盒的是 `weak`，都不是的护得住。
fn reason(mounts: bool, strength: &str) -> Option<&'static str> {
    if !mounts {
        Some("no_bwrap")
    } else if strength == WEAK {
        Some("weak")
    } else {
        None
    }
}

#[cfg(test)]
mod tests;

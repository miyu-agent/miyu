//! 软件包的状态、开关、后台页（施工 F-6 上，`docs/blueprint/package-pages.md`，设计 `30-插件框架.md` 第十三节）：`package.list`
//! 每一项多的 `status`、`enabled`、`page` 照这里算；程序不在的扩展、小程序当没装（配置项、功能都不算），也照这里认。

use std::path::{Path, PathBuf};

use miyu_config::package::{Manifest, Mascot, PackageKind};
use miyu_store::packages::{Found, locate};

use crate::Core;
use crate::extensions::State;

/// 包的程序叫什么：扩展、界面照 `[command]`，小程序照 `[worker]`；内置包、吉祥物包、`bundle` 没有程序（`bundle` 带的 MCP
/// 服务照它自己的命令起，施工 X-3）。
fn program(manifest: &Manifest) -> Option<&str> {
    match manifest.kind {
        PackageKind::Process | PackageKind::Ui => manifest
            .command
            .as_ref()
            .map(|command| command.program.as_str()),
        PackageKind::Worker => manifest
            .worker
            .as_ref()
            .map(|worker| worker.program.as_str()),
        PackageKind::Builtin | PackageKind::Mascot | PackageKind::Bundle => None,
    }
}

/// 主程序在哪：包的程序找它旁边的（`packages.md`「转交」）。测试里是测试程序自己。
fn main_program() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"))
}

/// 要程序的包，程序不在包目录 `dir` 的 `bin/` 里、也不在 `miyu` 旁边（`miyu_store::packages::locate`，施工 F-8 上再补）。
pub(crate) fn program_missing(dir: &Path, manifest: &Manifest) -> bool {
    program(manifest).is_some_and(|program| locate(program, dir, &main_program()).is_none())
}

/// 当没装（`package-pages.md`「程序不在就当没装」）：读成了的、核心拉起的那两种（扩展、小程序），程序不在。界面不算：它不是
/// 核心拉起的，连得上核心的界面自己就是程序在的证明（终端的会话 2026-10-10 提：开发时界面的程序不在 `miyu` 旁边）。
pub fn absent(found: &Found) -> bool {
    found.read.as_ref().is_ok_and(|manifest| {
        matches!(manifest.kind, PackageKind::Process | PackageKind::Worker)
            && program_missing(&found.files_dir(), manifest)
    })
}

/// 有没有后台页：清单写了 `[page]`，包目录里那个子目录下有 `index.html`。
pub(crate) fn page(found: &Found, manifest: &Manifest) -> bool {
    manifest
        .page
        .as_ref()
        .is_some_and(|dir| found.files_dir().join(dir).join("index.html").is_file())
}

/// 吉祥物包的模型文件最大几个字节（施工 F-7）。
const MASCOT_MAX: u64 = 256 * 1024;

/// 吉祥物包的模型文件（施工 F-7，`packages.md`「吉祥物包」）：不在（照真实路径找，顺着链接出了包目录的也算不在）、超过
/// 256 KiB、不是 JSON 的对象，交回 `miyu check` 报的代码；好的没有。模型里面写得对不对由终端查。
pub(crate) fn mascot_problem(found: &Found, mascot: &Mascot) -> Option<&'static str> {
    let root = std::fs::canonicalize(found.files_dir()).ok();
    let real = std::fs::canonicalize(found.files_dir().join(&mascot.model)).ok();
    let Some(real) = real
        .filter(|real| root.as_ref().is_some_and(|root| real.starts_with(root)) && real.is_file())
    else {
        return Some("mascot_missing");
    };
    if std::fs::metadata(&real).map_or(true, |meta| meta.len() > MASCOT_MAX) {
        return Some("mascot_too_big");
    }
    let object = std::fs::read(&real)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|value| value.is_object());
    (!object).then_some("mascot_not_json")
}

/// `status`：`program_missing`、`off`、`running`、`starting`、`stopped`、`ready` 之一。`removed` 是出厂的包卸掉了。`bundle`
/// 开着的照它带的服务合起来算（[`crate::mcp::Servers::state`]）。
pub(crate) fn status(
    core: &Core,
    found: &Found,
    manifest: &Manifest,
    removed: bool,
) -> &'static str {
    let id = found.id.as_str();
    if program_missing(&found.files_dir(), manifest) {
        return "program_missing";
    }
    if removed {
        return "off";
    }
    if !matches!(manifest.kind, PackageKind::Process | PackageKind::Bundle) {
        return "ready";
    }
    if !crate::extensions::switched_on(core, id, manifest) {
        return "off";
    }
    match manifest.kind {
        // `bundle` 照它带的服务合起来算（施工 X-3 上）。
        PackageKind::Bundle => of_state(core.mcp.state(id, manifest).unwrap_or(State::Off)),
        _ => of_state(core.extensions.status(id).state),
    }
}

/// 开着的扩展这时在哪一步，写成列表上的状态：退避中的等着再拉起，算启动中。
fn of_state(state: State) -> &'static str {
    match state {
        State::Off => "off",
        State::Starting { .. } | State::Waiting { .. } => "starting",
        State::Running { .. } => "running",
        State::Stopped(_) => "stopped",
    }
}

/// `enabled`：有开关的才有（`package-pages.md`「开关」）。扩展、`bundle`（施工 X-3 上）照它的开关；出厂的、不是必需的内置包
/// 照有没有卸掉；必需的、界面、小程序、吉祥物包没有。`shipped` 是出厂的那一层。
pub(crate) fn enabled(
    core: &Core,
    id: &str,
    manifest: &Manifest,
    shipped: bool,
    removed: bool,
) -> Option<bool> {
    match manifest.kind {
        PackageKind::Process | PackageKind::Bundle => {
            Some(!removed && crate::extensions::switched_on(core, id, manifest))
        }
        PackageKind::Builtin if shipped && !manifest.required => Some(!removed),
        PackageKind::Builtin | PackageKind::Ui | PackageKind::Worker | PackageKind::Mascot => None,
    }
}

#[cfg(test)]
mod tests;

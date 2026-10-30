//! 这台机器上用不上的配置项（施工 UH-1，2026-10-31 项目主人在 macOS 上只装了桌面端，设置里照样出现「默认界面」和终端集成会话
//! 那几项）：`config.schema` 照装了哪些界面标 `hidden`，头本来就不画 `hidden` 的，协议不加格。值照旧能写、能查。
//!
//! - 终端集成会话那三项（`terminal.*`）：装了终端界面 [`TUI`] 才画。
//! - 默认界面 `ui.head`（直接敲 `miyu` 打开哪个界面）：装了两个以上界面才画，只有一个的没得选。

use miyu_config::package::PackageKind;
use miyu_store::packages::Found;

/// 终端界面的包。
const TUI: &str = "miyu-tui";

/// 装着的界面包的编号（种类是界面的，读不成的不算）。
pub(super) fn heads(found: &[Found]) -> Vec<String> {
    crate::packages::manifests(found)
        .filter(|(_, manifest)| manifest.kind == PackageKind::Ui)
        .map(|(id, _)| id.to_string())
        .collect()
}

/// 装着界面 `heads` 的这台机器上，`key` 这一项用不上。
pub(super) fn unused(key: &str, heads: &[String]) -> bool {
    if key.starts_with("terminal.") {
        return !heads.iter().any(|head| head == TUI);
    }
    key == "ui.head" && heads.len() < 2
}

#[cfg(test)]
mod tests;

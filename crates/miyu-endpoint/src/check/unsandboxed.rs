//! 管理员放行出沙盒的工具（施工 X-36，`11-权限与沙盒.md` A15）：系统配置的 `sandbox.tools.<名字>` 每一条照现在的工具目录查。
//! 目录里没有这件的（`sandbox_tool_missing`）、写的来处和这件工具现在归的包对不上的（`sandbox_tool_moved`，印出现在的来处，
//! 没错的话照抄），文件不在管理员管得住的地方的（`sandbox_tool_linked`，施工 X-37、X-37 补：文件、所在的目录是指出去的链接，
//! 印出真实位置），这三种不算放行；放行了 `shell` 的（`sandbox_shell`）照算，提醒一声：她跑的每条命令都在沙盒外。都是警告。
//! 落没落在她写得了的地方要看会话的工作区，这里不查，会话判放行时查。

use serde_json::{Value, json};

use miyu_config::Words;
use miyu_session::unsandboxed::{Confines, Unsandboxed};
use miyu_store::human::Human;

use super::shown;
use crate::Core;

/// 几条警告：`kind` 是 `config`，`file` 是系统配置。
pub(super) fn unsandboxed(core: &Core, words: &Human) -> Vec<Value> {
    let resolved = core.config_now().borrow().resolved().clone();
    let table = Unsandboxed::of(&resolved.values());
    let catalog = core.tools();
    let confines = Confines::of(&core.root, true, core.resources.path());
    let file = shown(core, &core.root.path().join("config.toml"));
    let warn = |code: &str, fields: &[(&str, &str)]| {
        let message =
            Words::sentence(words, &format!("package-problems/{code}"), fields).unwrap_or_default();
        json!({"kind": "config", "file": file, "code": code, "level": "warning", "message": message})
    };
    let mut problems = Vec::new();
    for (name, wrote) in table.entries() {
        match catalog.package_of(name) {
            None => problems.push(warn("sandbox_tool_missing", &[("name", name)])),
            Some(now) if now != wrote => problems.push(warn(
                "sandbox_tool_moved",
                &[("name", name), ("wrote", wrote), ("now", now)],
            )),
            Some(_)
                if let Some(file) = catalog.get(name).and_then(|tool| tool.file())
                    && confines.hold(file).is_none() =>
            {
                let real = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
                let real = real.display().to_string();
                problems.push(warn(
                    "sandbox_tool_linked",
                    &[("name", name), ("real", &real)],
                ));
            }
            Some(_) if catalog.get(name).is_some_and(|tool| tool.runs_commands()) => {
                problems.push(warn("sandbox_shell", &[]));
            }
            Some(_) => {}
        }
    }
    problems
}

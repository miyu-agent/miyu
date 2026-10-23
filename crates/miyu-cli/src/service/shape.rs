//! `miyu service … --format json` 的形状（施工 Z-1 下补，`docs/blueprint/cli/service.md`「机读格式」）：给头照着画开关，字段名
//! 稳定、不随界面语言变。成了的都是同一个形状 `{"installed", "on", "file", "manager"}`；没成的是 `{"error", "step", "said"}`。

use serde_json::{Value, json};

use super::{Manager, Problem, Said};

/// 要说的那一句写成 JSON。
pub(crate) fn shaped(said: &Said) -> Value {
    match said {
        // 装好了：管理器那边开着；自启动问不了，是 null。
        Said::Enabled(manager, file) => {
            state(*manager, file, true, asked(*manager).then_some(true))
        }
        Said::Disabled(manager, file) | Said::NotEnabled(manager, file) => {
            state(*manager, file, false, asked(*manager).then_some(false))
        }
        Said::Status {
            manager,
            file,
            installed,
            on,
        } => {
            // 没装的就是没开；装了、问不了的照旧是 null。
            let on = match installed {
                true => *on,
                false => asked(*manager).then_some(false),
            };
            state(*manager, file, *installed, on)
        }
        Said::Cannot(Problem::DataRootOnWindows) => json!({"error": "data_root_on_windows"}),
        Said::NoHome => json!({"error": "no_home"}),
        Said::Failed(step, said) => json!({"error": "failed", "step": step, "said": said}),
    }
}

/// 成了的那一个形状。
fn state(manager: Manager, file: &std::path::Path, installed: bool, on: Option<bool>) -> Value {
    json!({
        "installed": installed,
        "on": on,
        "file": file.display().to_string(),
        "manager": manager.code(),
    })
}

/// 问得了开没开的管理器：自启动问不了。
fn asked(manager: Manager) -> bool {
    manager != Manager::Autostart
}

#[cfg(test)]
mod tests;

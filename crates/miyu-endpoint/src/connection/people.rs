//! 只给人用的方法（从 `connection.rs` 挪出来：那边到了行数上限）。

/// 只给人用、扩展进程调了回 `local_only` 的方法：开关、重启扩展（施工 9-4 上），改、删人格和预设（施工 P-3 中），装、卸软件包
/// （施工 F-5 上），软件包的开关（施工 F-6 上），后台页的文件和方法（施工 F-6 中），技能、脚本、MCP 三页改加载方式、全局开关（施工 X-27），删技能（施工 W-13），改配置、信任项目配置、存删密钥（施工 T-9：
/// 改了能放宽权限、换模型、拿走别的软件的 key），项目（施工 H-1）。读配置、列密钥的名字照旧给。`config.set` 不在这里：它自己查，扩展只能改自己包
/// 编号开头的几项（施工 T-9 补，`config::set::set_from_package`）。
pub(super) fn people_only(method: &str) -> bool {
    method.starts_with("extension.")
        || method.starts_with("project.")
        || matches!(
            method,
            "preset.set"
                | "preset.delete"
                | "persona.set"
                | "persona.delete"
                | "package.install"
                | "package.remove"
                | "package.enable"
                | "package.disable"
                | "package.restart"
                | "package.sync"
                | "package.search"
                | "package.clean"
                | "package.upgrades"
                | "package.upgrade"
                | "package.file"
                | "mcp.list"
                | "mcp.restart"
                | "mcp.set"
                | "mcp.remove"
                | "skill.remove"
                | "tool.set_loading"
                | "feature.set"
                | "config.trust"
                | "secret.set"
                | "secret.delete"
        )
}

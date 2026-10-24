//! 管理员模式（施工 Y-12 下，设计 06 八再补第 2 条、U25）：改系统那一层的要管理员组的人进入管理员模式（请求外壳带
//! `"scope": "system"`）；平时管理员和别人一样只改自己那一份，另外读得到这台电脑的几样。2026-10-24 项目主人定：本机也要切，
//! 账号管理也在管理员模式下。
//!
//! 只对握手时声明了认识管理员模式（`caps.scope`）的连接要求：没声明的老头照以前的意思（协议 1 冻结：已有的请求不改意思）。

use serde_json::Value;

/// 改系统那一层的方法：账号管理、系统装的包和扩展、MCP 服务、供应商改名。
const SYSTEM: &[&str] = &[
    "account.invite",
    "account.invites",
    "account.invite_revoke",
    "account.list",
    "account.disable",
    "account.enable",
    "account.set_admin",
    "package.install",
    "package.remove",
    "package.upgrade",
    "package.enable",
    "package.disable",
    "package.clean",
    "package.sync",
    "package.restart",
    "extension.enable",
    "extension.disable",
    "extension.restart",
    "mcp.set",
    "mcp.remove",
    "mcp.login",
    "mcp.login_finish",
    "mcp.logout",
    "mcp.restart",
    "provider.rename",
];

/// 机主以外的管理员平时多读得到的（成员名单以外）：这台电脑的文件、链接卡片、一次性调模型、认供应商、软件包、扩展、MCP 的
/// 读（U25 第 2 条：读这台电脑的只给本机令牌的人和管理员组）。
const READS: &[&str] = &[
    "fs.find",
    "fs.list",
    "fs.read",
    "fs.realpath",
    "link.preview",
    "model.call",
    "provider.detect",
    "package.list",
    "package.info",
    "package.files",
    "package.file",
    "package.check",
    "package.owns",
    "package.search",
    "package.upgrades",
    "package.methods",
    "package.progress",
    "extension.status",
    "mcp.list",
];

/// `method`（参数 `params`）改不改系统那一层：上面那几个，`config.set` 写系统配置，`secret.set`、`secret.delete` 写系统
/// 那一份。
pub(super) fn touches_system(method: &str, params: &Value) -> bool {
    let layer_is_system = || params.get("layer").and_then(Value::as_str) == Some("system");
    SYSTEM.contains(&method)
        || (matches!(method, "config.set" | "secret.set" | "secret.delete") && layer_is_system())
}

/// 机主以外的管理员平时多读得到的：上面那几个，和订阅软件包、扩展的流（列得出就该看得到它变）。
pub(super) fn reads(method: &str, params: &Value) -> bool {
    READS.contains(&method)
        || (method == "subscribe"
            && matches!(
                params.get("stream").and_then(Value::as_str),
                Some("packages" | "extensions")
            ))
}

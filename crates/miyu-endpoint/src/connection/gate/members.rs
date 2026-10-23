//! 成员（不是管理员的人）能调什么（施工 Y-2 下下上，设计 06 U25「每一条路过同一道检查」）：照名单放，名单以外的一律
//! `not_permitted`。名单里的都只碰得到他自己的：会话照属主挡（`checks.rs` 的第 4 道），人格、预设、知识库、技能、脚本、项目、
//! 附件、记忆照「替谁干活」找（施工 Y-2、Y-7），会话里的命令记在他名下（[`crate::sessions::person`]）。
//!
//! 配置照人分开以后（施工 Y-2 下下中上）放读配置、查、配置清单、开关、加载方式、信任和订阅配置：都是他自己那一份。不在名单里
//! 的：写配置（`config.set`，同密钥照人分开一起放，Y-2 下下中中）、MCP、模型列表、一次性调模型；整台机器的事（装包、扩展、
//! 密钥、供应商、读本机文件、诊断）只给管理员。`link.preview` 从核心往外连，先不给（成员上网的规矩见 MB-2）。

use serde_json::Value;

/// 成员能调的方法（`subscribe` 另照流挑，见 [`allowed`]）。
const METHODS: &[&str] = &[
    // 连接本身、自己的登录。
    "hello",
    "human.get",
    "unsubscribe",
    "account.logout",
    "login.ticket",
    "login.redeem",
    // 自己的会话。
    "session.list",
    "session.create",
    "session.delete",
    "session.send",
    "session.interrupt",
    "session.withdraw",
    "session.revert",
    "session.unrevert",
    "session.redo",
    "session.compact",
    "session.clear",
    "session.configure",
    "session.set_meta",
    "session.answer",
    "session.recap",
    "session.fork",
    "session.set_workspace",
    "session.set_permission_level",
    "session.tools",
    "session.context",
    "session.usage_parts",
    "session.terminal",
    "session.set_terminal",
    "job.stop",
    "job.output",
    "view.page",
    "view.detail",
    "command.run",
    "command.catalog",
    // 自己的附件。
    "blob.put",
    "blob.open",
    "blob.write",
    "blob.close",
    "blob.get",
    // 自己的人格、预设、技能、脚本、项目。
    "persona.list",
    "persona.get",
    "persona.read",
    "persona.avatar",
    "persona.background",
    "persona.set",
    "persona.delete",
    "preset.list",
    "preset.get",
    "preset.set",
    "preset.delete",
    "skill.list",
    "skill.remove",
    "script.list",
    "project.list",
    "project.create",
    "project.update",
    "project.delete",
    // 自己的知识库、记忆（施工 Y-7）、用量。
    "kb.list",
    "kb.search",
    "kb.documents",
    "kb.read",
    "kb.add",
    "kb.create",
    "kb.update",
    "kb.mount",
    "kb.remove",
    "kb.delete",
    "kb.reindex",
    "memory.list",
    "memory.search",
    "memory.turns",
    "memory.remember",
    "memory.update",
    "memory.forget",
    "memory.restore",
    "memory.dream",
    "memory.dream_stop",
    "memory.cards",
    "memory.card_update",
    "memory.card_forget",
    "usage.query",
    // 他自己的配置（施工 Y-2 下下中上）：读、查、清单、功能开关、加载方式、项目配置信不信任。
    "config.get",
    "config.check",
    "config.schema",
    "feature.set",
    "tool.set_loading",
    "config.trust",
    // 不碰谁的：供应商的出厂目录、画图。
    "provider.catalog",
    "mermaid.render",
];

/// 成员订得了的流：自己的会话、事件、视图、项目、知识库、记忆、界面块、他自己的配置（施工 Y-2 下下中上）。扩展、软件包的不给。
const STREAMS: &[&str] = &[
    "events", "view", "sessions", "projects", "kb", "memory", "widgets", "config",
];

/// 成员能不能调 `method`（参数 `params`）。
pub(super) fn allowed(method: &str, params: &Value) -> bool {
    if method == "subscribe" {
        return params
            .get("stream")
            .and_then(Value::as_str)
            .is_some_and(|stream| STREAMS.contains(&stream));
    }
    METHODS.contains(&method)
}

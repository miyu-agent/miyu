//! 核心拉起的扩展能调什么（施工 X-30，`docs/blueprint/extensions.md`「谁能调」，`05-内核接口.md` 第三节、I5）：每个方法照这张表
//! 挡，三种：只给人的回 `local_only`；要某项能力的，清单没声明就回 `needs_approval`，`data.capabilities` 是缺的那一项；都能调的
//! 照常。批过的就是声明的：还有没批的扩展不拉起（`extensions.md`「能力」第 4 条），所以照清单的 `[process] capabilities` 算。
//!
//! 表里没有的方法当只给人：新方法默认不对扩展开，加方法的人要在这里写明谁能调。门禁的「文档」一项查代码里分发的方法都在这张表里
//! （`xtask/src/rights.rs`）。头的连接不经这里。

use serde_json::{Value, json};

use miyu_config::package::Capability;

use crate::Core;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 扩展调一个方法要什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Right {
    /// 只给人：改人的设置、权限、信任，替人回答，装卸软件。
    People,
    /// 要清单声明了这项能力。
    Needs(Capability),
    /// 都能调。
    Anyone,
}

/// 扩展调 `method`（参数 `params`）要什么。有几样照参数分：带 `as` 的是代表平台上的人，`venue.*` 的事件只给桥，`provide` 里有工具
/// 的才要 `tools`（空的照收：斜杠命令、搜索要先 `provide` 一次），订阅照流分。
pub(super) fn right(method: &str, params: &Value) -> Right {
    use Capability::{
        ActForExternal, ContextSystem, EventsRead, EventsWrite, FsRead, MemoryWrite, Network,
        PackagesCall, SessionsDrive, Tools, Widgets,
    };
    use Right::{Anyone, Needs, People};
    let acting = params.get("as").is_some_and(|acting| !acting.is_null());
    match method {
        "session.send" | "command.run" if acting => Needs(ActForExternal),
        "events.append"
            if params
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.starts_with("venue.")) =>
        {
            Needs(ActForExternal)
        }
        "provide"
            if params
                .get("tools")
                .and_then(Value::as_array)
                .is_some_and(|tools| !tools.is_empty()) =>
        {
            Needs(Tools)
        }
        "subscribe" => match params.get("stream").and_then(Value::as_str) {
            Some("events" | "view" | "sessions") => Needs(EventsRead),
            Some("projects") => People,
            _ => Anyone,
        },
        // 驱动会话。
        "session.create" | "session.send" | "session.interrupt" | "session.withdraw"
        | "session.revert" | "session.unrevert" | "session.redo" | "session.compact"
        | "session.clear" | "session.configure" | "session.fork" | "session.set_meta"
        | "session.respond" | "command.run" | "job.stop" => Needs(SessionsDrive),
        // 删会话（施工 Y-2 上，项目主人 10-23 定场所会话的删放在平台的后台）：只删得掉自己名下的，属主照 `checks.rs` 查。
        "session.delete" => Needs(SessionsDrive),
        // 读会话里的东西。
        "session.list"
        | "session.recap"
        | "session.tools"
        | "session.context"
        | "session.usage_parts"
        | "view.page"
        | "view.detail"
        | "job.output" => Needs(EventsRead),
        "events.append" => Needs(EventsWrite),
        // 代表平台上的人、场所的原语（`venue.delivered`、`venue.recalled` 是 `events.append` 里只给桥的两种事件）。
        "session.note" | "venue.binding" | "venue.delivered" | "venue.recalled"
        | "venue.records" | "venue.session" | "venue.sessions" => Needs(ActForExternal),
        "memory.remember" | "memory.update" | "memory.forget" | "memory.restore"
        | "memory.dream" | "memory.dream_stop" | "memory.card_update" | "memory.card_forget"
        | "kb.add" | "kb.create" | "kb.update" | "kb.mount" | "kb.remove" | "kb.delete"
        | "kb.reindex" => Needs(MemoryWrite),
        "fs.list" | "fs.find" | "fs.realpath" | "fs.read" => Needs(FsRead),
        "link.preview" => Needs(Network),
        "widget.set" | "widget.clear" => Needs(Widgets),
        "system.append" => Needs(ContextSystem),
        "package.call" => Needs(PackagesCall),
        // 都能调：读配置和密钥的名字、附件、模型、给人看的字、装了什么、带来的内容、人格和预设、技能、脚本、记忆和知识库读的。
        "hello" | "unsubscribe" | "provide" | "human.get" | "blob.put" | "blob.get"
        | "blob.open" | "blob.write" | "blob.close" | "command.catalog" | "config.check"
        | "config.get" | "config.schema" | "config.set" | "secret.list" | "content.list"
        | "content.read" | "kb.documents" | "kb.list" | "kb.read" | "kb.search"
        | "login.redeem" | "memory.cards" | "memory.list" | "memory.search" | "memory.turns"
        | "mermaid.render" | "model.call" | "model.embed" | "model.list" | "package.check"
        | "package.files" | "package.info" | "package.list" | "package.methods"
        | "package.owns" | "persona.avatar" | "persona.background" | "persona.get"
        | "persona.list" | "persona.read" | "preset.get" | "preset.list" | "provider.catalog"
        | "provider.detect" | "provider.test" | "script.list" | "skill.list" | "usage.query" => {
            Anyone
        }
        // 只给人：替人回答、改权限和工作目录、终端会话、账号和登录、改人格预设、装卸软件包、改配置和密钥、项目。
        "session.answer"
        | "session.set_permission_level"
        | "session.set_workspace"
        | "session.terminal"
        | "session.set_terminal"
        | "account.disable"
        | "account.enable"
        | "account.invite"
        | "account.invite_revoke"
        | "account.invites"
        | "account.list"
        | "account.logout"
        | "account.register"
        | "account.setup"
        | "account.setup_code"
        | "login.ticket"
        | "preset.set"
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
        | "mcp.login"
        | "mcp.login_finish"
        | "mcp.logout"
        | "mcp.set"
        | "mcp.remove"
        | "skill.remove"
        | "tool.set_loading"
        | "feature.set"
        | "config.trust"
        | "secret.set"
        | "provider.rename"
        | "secret.delete"
        | "extension.enable"
        | "extension.disable"
        | "extension.restart"
        | "extension.status"
        | "project.list"
        | "project.create"
        | "project.update"
        | "project.delete" => People,
        // 核心发出去的（反向调用、推送），扩展调了没有意义；写明了门禁才认得全。
        "tool.call" | "tool.cancel" | "command.call" | "method.call" | "search.query"
        | "user.append" | "widget.watch" | "widget.unwatch" | "view.add" | "view.append"
        | "view.update" | "view.remove" | "view.hidden" | "package.progress" => People,
        _ => People,
    }
}

/// 包 `package` 清单里声明的能力（批过的就是这些，见模块的说明）；读不成的、不是扩展的没有。
pub(super) fn declared(core: &Core, package: &str) -> Vec<Capability> {
    core.packages()
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok())
        .and_then(|manifest| manifest.process.as_ref())
        .map(|process| process.capabilities.clone())
        .unwrap_or_default()
}

/// 扩展的这一条请求被挡的话，挡的那一句；放行的没有。
pub(super) fn refuse(request: &Request, declared: &[Capability]) -> Option<Refusal> {
    match right(&request.method, &request.params) {
        Right::People => Some(Refusal::LOCAL_ONLY),
        Right::Needs(capability) if !declared.contains(&capability) => {
            Some(Refusal::needs_approval(json!([capability.as_str()])))
        }
        Right::Needs(_) | Right::Anyone => None,
    }
}

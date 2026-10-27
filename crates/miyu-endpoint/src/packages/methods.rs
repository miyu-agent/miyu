//! 软件包的几个方法（`protocol.md` 的 `package.*`）：从端点的方法表挪出来（施工 F-8 中上，方法表放不下了），照方法名分给各处。
//! 在后台答的 `package.call` 不在这里（`methods::call_later`）。个人设置里的 MCP 服务的 `mcp.*`（施工 F-9 补）也从这里分：
//! 方法表到了行数上限。`mcp.set`、`mcp.remove`、列技能的 `skill.list`（施工 X-26），列脚本的 `script.list`、改加载方式的
//! `tool.set_loading`、改全局开关的 `feature.set`（施工 X-27）同样从这里分。

use std::sync::Arc;

use serde_json::Value;

use super::scope::Scope;
use crate::Core;
use crate::hello::{Caller, Peer};
use crate::methods::params;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 答一条 `package.*`、`mcp.*`、`skill.*`、`script.*`、`tool.*`、`feature.*`；不认识的 `unknown_method`。
pub(crate) async fn call(
    core: &Arc<Core>,
    peer: Peer,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "package.list" => super::list(core, peer, caller.acting_for()),
        // 从软件源装（施工 G-4 上、下）：带 `source` 的，只写 `package`、又不是卸掉的出厂包的；要往发起的连接推下载进度。装、卸、
        // 升级进哪一层照请求在不在管理员模式里（施工 U23 下上，`scope.rs`）。
        "package.install" if super::source::fetch::wanted(core, &request.params) => {
            let scope = Scope::of(peer, caller, request);
            let reverse = Some(&caller.reverse);
            super::source::fetch::install(core, (peer, &scope), reverse, params(request)?).await
        }
        // 写组名的（施工 BS-1 下中）：整组装回、卸掉，只有系统那一层。
        "package.install" | "package.remove" if super::group::named(core, &request.params) => {
            Scope::of(peer, caller, request).system_only(peer.admin)?;
            super::group::call(core, peer, &request.method, params(request)?).await
        }
        "package.install" => {
            let scope = Scope::of(peer, caller, request);
            super::asked::install(core, peer, &scope, params(request)?).await
        }
        "package.remove" => {
            let scope = Scope::of(peer, caller, request);
            super::asked::remove(core, peer, &scope, params(request)?).await
        }
        "package.info" => {
            let whose = (caller.acting_for(), peer.admin);
            super::local::info(core, whose, params(request)?).await
        }
        "package.files" => super::local::files(core, caller.acting_for(), params(request)?).await,
        "package.owns" => super::verify::owns(core, params(request)?).await,
        "package.check" => super::verify::check(core, params(request)?).await,
        "package.sync" => super::source::sync::method(core, params(request)?).await,
        "package.search" => {
            let whose = (peer, caller.acting_for());
            super::source::search::search(core, whose, params(request)?).await
        }
        "package.clean" => super::source::clean::clean(core).await,
        "package.upgrades" => {
            super::source::upgrade::upgrades(core, &Scope::of(peer, caller, request)).await
        }
        "package.upgrade" => {
            let scope = Scope::of(peer, caller, request);
            let reverse = Some(&caller.reverse);
            super::source::upgrade::upgrade(core, (peer, &scope), reverse, params(request)?).await
        }
        "package.enable" => {
            super::switch::enable(core, (peer, caller.acting_for()), params(request)?).await
        }
        "package.disable" => {
            super::switch::disable(core, (peer, caller.acting_for()), params(request)?).await
        }
        "package.restart" => {
            super::switch::restart(core, (peer, caller.acting_for()), params(request)?).await
        }
        "package.members" => {
            super::switch::members(core, (peer, caller.acting_for()), params(request)?).await
        }
        "package.file" => crate::backstage::read(core, caller, params(request)?).await,
        "package.emit" => crate::backstage::emit(core, caller, params(request)?),
        "package.methods" => crate::backstage::register(core, caller, params(request)?),
        "widget.set" => crate::widgets::set::set(core, caller, params(request)?),
        "widget.clear" => crate::widgets::set::clear(core, caller, params(request)?),
        "mcp.list" => Ok(crate::mcp::methods::list(core)),
        "mcp.restart" => crate::mcp::methods::restart(core, params(request)?).await,
        "mcp.login" => crate::mcp::login::login(core, peer, &request.id, params(request)?).await,
        "mcp.login_finish" => crate::mcp::login::finish(core, params(request)?).await,
        "mcp.logout" => crate::mcp::login::logout(core, peer, &request.id, params(request)?),
        "mcp.set" => crate::mcp::editing::set(core, peer, &request.id, params(request)?),
        "mcp.remove" => crate::mcp::editing::remove(core, peer, &request.id, params(request)?),
        "skill.list" => crate::skills::list(core, caller.acting_for(), peer).await,
        "skill.remove" => crate::skills::remove(core, caller.acting_for(), params(request)?).await,
        "script.list" => Ok(crate::scripts::list::list(core, caller.acting_for(), peer).await),
        "tool.set_loading" => crate::toggles::set_loading(&params(request)?),
        "feature.set" => {
            crate::toggles::set_feature(core, (caller, peer), &request.id, params(request)?)
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

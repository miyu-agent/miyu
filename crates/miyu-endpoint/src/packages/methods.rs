//! 软件包的几个方法（`protocol.md` 的 `package.*`）：从端点的方法表挪出来（施工 F-8 中上，方法表放不下了），照方法名分给各处。
//! 在后台答的 `package.call` 不在这里（`methods::call_later`）。配置里的 MCP 服务的 `mcp.*`（施工 F-9 补）也从这里分：
//! 方法表到了行数上限。`mcp.set`、`mcp.remove`、列技能的 `skill.list`（施工 X-26），列脚本的 `script.list`、改全局开关的
//! `feature.set`（施工 X-27）同样从这里分。

use std::sync::Arc;

use crate::Core;
use crate::hello::{Caller, Peer};
use crate::methods::{Answer, answer, params};
use crate::refusal::Refusal;
use crate::wire::Request;

/// 答一条 `package.*`、`mcp.*`、`skill.*`、`script.*`、`tool.*`、`feature.*`；不认识的 `unknown_method`。
pub(crate) fn call<'a>(
    core: &'a Arc<Core>,
    peer: Peer,
    caller: &'a Caller,
    request: &'a Request,
) -> Answer<'a> {
    // 每一种方法各是一个放在堆上的 future（施工 LW-8）：原来整个是一个 async 函数，几十个方法的身子并进一个 63 kB 的状态机，
    // 答一次 `package.list` 也算执行过整块、跟着留在内存里（`perf.md`「排函数」）。
    match request.method.as_str() {
        "package.list" => answer(move || async move { super::list(core, peer) }),
        // 从软件源装（施工 G-4 上、下）：带 `source` 的，只写 `package`、又不是卸掉的出厂包的；要往发起的连接推下载进度。装、卸、
        // 升级只给人（`connection/rights.rs`），人就是主人（施工 SU-2 上）。
        "package.install" if super::source::fetch::wanted(core, &request.params) => {
            answer(move || async move {
                let reverse = Some(&caller.reverse);
                super::source::fetch::install(core, peer, reverse, params(request)?).await
            })
        }
        // 写组名的（施工 BS-1 下中）：整组装回、卸掉。
        "package.install" | "package.remove" if super::group::named(core, &request.params) => {
            answer(move || async move {
                super::group::call(core, peer, &request.method, params(request)?).await
            })
        }
        "package.install" => answer(move || async move {
            let reverse = Some(&caller.reverse);
            super::asked::install(core, peer, reverse, params(request)?).await
        }),
        "package.remove" => {
            answer(move || async move { super::asked::remove(core, params(request)?).await })
        }
        "package.info" => {
            answer(move || async move { super::info::info(core, peer, params(request)?).await })
        }
        "package.files" => {
            answer(move || async move { super::local::files(core, params(request)?).await })
        }
        "package.logo" => {
            answer(move || async move { super::logo::method(core, params(request)?).await })
        }
        "package.owns" => {
            answer(move || async move { super::verify::owns(core, params(request)?).await })
        }
        "package.check" => {
            answer(move || async move { super::verify::check(core, params(request)?).await })
        }
        "package.sync" => {
            answer(move || async move { super::source::sync::method(core, params(request)?).await })
        }
        "package.search" => answer(move || async move {
            super::source::search::search(core, peer, params(request)?).await
        }),
        "package.clean" => answer(move || async move { super::source::clean::clean(core).await }),
        "package.upgrades" => {
            answer(move || async move { super::source::upgrade::upgrades(core).await })
        }
        "package.upgrade" => answer(move || async move {
            let reverse = Some(&caller.reverse);
            super::source::upgrade::upgrade(core, peer, reverse, params(request)?).await
        }),
        "package.enable" => {
            answer(move || async move { super::switch::enable(core, peer, params(request)?).await })
        }
        "package.disable" => {
            answer(
                move || async move { super::switch::disable(core, peer, params(request)?).await },
            )
        }
        "package.restart" => {
            answer(
                move || async move { super::switch::restart(core, peer, params(request)?).await },
            )
        }
        "package.file" => {
            answer(move || async move { crate::backstage::read(core, params(request)?).await })
        }
        "package.page" => answer(move || async move {
            crate::backstage::host::open(core, peer, params(request)?).await
        }),
        "package.emit" => {
            answer(move || async move { crate::backstage::emit(core, caller, params(request)?) })
        }
        "package.methods" => {
            answer(
                move || async move { crate::backstage::register(core, caller, params(request)?) },
            )
        }
        "widget.set" => {
            answer(move || async move { crate::widgets::set::set(core, caller, params(request)?) })
        }
        "widget.clear" => {
            answer(
                move || async move { crate::widgets::set::clear(core, caller, params(request)?) },
            )
        }
        "mcp.list" => answer(move || async move { Ok(crate::mcp::methods::list(core)) }),
        "mcp.restart" => {
            answer(
                move || async move { crate::mcp::methods::restart(core, params(request)?).await },
            )
        }
        "mcp.login" => answer(move || async move {
            crate::mcp::login::login(core, peer, &request.id, params(request)?).await
        }),
        "mcp.login_finish" => {
            answer(move || async move { crate::mcp::login::finish(core, params(request)?).await })
        }
        "mcp.logout" => answer(move || async move {
            crate::mcp::login::logout(core, peer, &request.id, params(request)?)
        }),
        "mcp.set" => answer(move || async move {
            crate::mcp::editing::set(core, peer, &request.id, params(request)?)
        }),
        "mcp.remove" => answer(move || async move {
            crate::mcp::editing::remove(core, peer, &request.id, params(request)?)
        }),
        "skill.list" => answer(move || async move { crate::skills::list(core, peer).await }),
        "skill.remove" => {
            answer(move || async move { crate::skills::remove(core, params(request)?).await })
        }
        "script.list" => {
            answer(move || async move { Ok(crate::scripts::list::list(core, peer).await) })
        }
        "feature.offer" => {
            answer(move || async move { crate::extensions::offer(core, caller, params(request)?) })
        }
        "feature.set" => answer(move || async move {
            crate::toggles::set_feature(core, peer, &request.id, params(request)?)
        }),
        _ => answer(move || async move { Err(Refusal::UNKNOWN_METHOD) }),
    }
}

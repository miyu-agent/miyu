//! 软件包的几个方法（`protocol.md` 的 `package.*`）：从端点的方法表挪出来（施工 F-8 中上，方法表放不下了），照方法名分给各处。
//! 在后台答的 `package.call` 不在这里（`methods::call_later`）。个人设置里的 MCP 服务的 `mcp.*`（施工 F-9 补）也从这里分：
//! 方法表到了行数上限。

use std::sync::Arc;

use serde_json::Value;

use crate::Core;
use crate::hello::{Caller, Peer};
use crate::methods::params;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 答一条 `package.*`、`mcp.*`；不认识的 `unknown_method`。
pub(crate) async fn call(
    core: &Arc<Core>,
    peer: Peer,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "package.list" => super::list(core, peer),
        "package.install" => super::manage::install(core, peer, params(request)?).await,
        "package.remove" => super::manage::remove(core, params(request)?).await,
        "package.info" => super::local::info(core, params(request)?).await,
        "package.files" => super::local::files(core, params(request)?).await,
        "package.owns" => super::verify::owns(core, params(request)?).await,
        "package.check" => super::verify::check(core, params(request)?).await,
        "package.enable" => super::switch::enable(core, peer, params(request)?).await,
        "package.disable" => super::switch::disable(core, peer, params(request)?).await,
        "package.restart" => super::switch::restart(core, peer, params(request)?).await,
        "package.file" => crate::backstage::read(core, params(request)?).await,
        "package.methods" => crate::backstage::register(core, caller, params(request)?),
        "mcp.list" => Ok(crate::mcp::methods::list(core)),
        "mcp.restart" => crate::mcp::methods::restart(core, params(request)?).await,
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

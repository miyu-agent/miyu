//! 软件包列表上每个 MCP 服务这时的状态（施工 X-4，`docs/blueprint/mcp.md`「头上的状态」）：`mcp` 的每一格同 `extension.status`
//! 的一项多 `state`、`failures`，在跑的带 `pid`，在等的带 `retry_in`，停下的带 `reason` 和标准错误的最后几行 `stderr`。服务的
//! 状态变了照扩展那样报一声（`mcp.rs` 的 `Shared::set`），订阅着软件包列表的连接收到这个包的新一项。每一格另带全局开关
//! `on`，同 `mcp.list` 的一项（施工 X-27 补，网页的 MCP 页要；加载方式 `need_load` 施工 FL-1 拆了）。

use serde_json::{Value, json};

use miyu_config::package::Manifest;

use crate::Core;
use crate::extensions::methods::details;

/// 给包 `id`（清单是 `manifest`）那一项的 `mcp` 每一格填上这时的状态、加载方式、全局开关：没有 `mcp` 的不动。服务全被
/// 配置里同名的盖住的，包那一项也带 `covered: true`（施工 F-9 补）。
pub(super) fn fill(core: &Core, id: &str, manifest: &Manifest, item: &mut Value) {
    let Some(servers) = item.get_mut("mcp").and_then(Value::as_array_mut) else {
        return;
    };
    let all_covered = !servers.is_empty()
        && servers.iter().all(|server| {
            server["name"]
                .as_str()
                .is_some_and(|name| core.home_mcp.covers(name))
        });
    for server in servers {
        let Some(name) = server["name"].as_str().map(str::to_string) else {
            continue;
        };
        let status = core.mcp.service(id, &name);
        if core.home_mcp.covers(&name) {
            // 被配置里同名的盖住了（施工 F-9 补）：不起，`state` 照旧是 `off`。
            server["covered"] = json!(true);
        }
        server["state"] = json!(status.state.as_str());
        server["failures"] = json!(status.failures);
        details(server, status.state, &crate::mcp::stderr_file(core, &name));
        crate::mcp::methods::skipped(core, id, &name, server);
        if let Some(written) = manifest.mcp.iter().find(|written| written.name == name) {
            crate::mcp::methods::toggles(core, written, server);
        }
    }
    if all_covered {
        item["covered"] = json!(true);
    }
}

//! `bundle` 的开关（施工 X-3 上，`packages.md`「MCP 服务」第 4 条）：`package.enable`、`package.disable` 走这里。开关和扩展记在
//! 同一份文件、同一把锁下一件件办；没写的关着。开了起它带的服务，关了停下、它们的工具出目录。

use std::sync::Arc;

use miyu_config::package::Manifest;

use super::TARGET;
use crate::Core;
use crate::extensions::{read_switches, write_switches};
use crate::refusal::Refusal;

/// 打开 `bundle` 包 `id`（清单是 `manifest`）：记成开着，起它带的服务。
pub(crate) async fn turn_on(
    core: &Arc<Core>,
    id: &str,
    manifest: &Manifest,
) -> Result<(), Refusal> {
    let _one_at_a_time = core.extensions.one_at_a_time().await;
    let (mut switches, version) = read_switches(core);
    switches.on.insert(id.to_string(), true);
    write_switches(core, id, &switches, version.as_deref())?;
    core.mcp.launch(core, id, manifest);
    core.extensions.notify(id);
    Ok(())
}

/// 关掉 `bundle` 包 `id`：记成关着，停下它带的服务，它们的工具出目录。
pub(crate) async fn turn_off(
    core: &Arc<Core>,
    id: &str,
    manifest: &Manifest,
) -> Result<(), Refusal> {
    let _one_at_a_time = core.extensions.one_at_a_time().await;
    let (mut switches, version) = read_switches(core);
    switches.on.insert(id.to_string(), false);
    write_switches(core, id, &switches, version.as_deref())?;
    core.mcp.halt(core, id).await;
    withdraw(core, id, manifest);
    core.extensions.notify(id);
    Ok(())
}

/// 包 `id` 带的服务的工具出目录（清单是 `manifest`）：开着的会话下一个回合换掉。
pub(super) fn withdraw(core: &Core, id: &str, manifest: &Manifest) {
    for server in &manifest.mcp {
        let service = server.name.as_str();
        let generation = core
            .tools
            .replace(|catalog| {
                Ok::<_, std::convert::Infallible>(
                    catalog.replacing_service(id, service, Vec::new()).0,
                )
            })
            .unwrap_or_else(|never| match never {});
        tracing::info!(target: TARGET, package = id, service, generation, "mcp tools withdrawn");
    }
}

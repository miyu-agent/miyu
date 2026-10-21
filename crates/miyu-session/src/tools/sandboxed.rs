//! 给调用写上沙盒（从 `tools.rs` 挪来，施工 M-2：那边要放嵌套调用的端口，过了 500 行）。直接调用、嵌套调用都经它。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::Permission;
use miyu_kernel::id::CallId;
use miyu_kernel::raw::RawJson;
use miyu_tool::Call;

use crate::TARGET;
use crate::blocking::blocking;
use crate::sandbox::Sandbox;

/// 照实际生效的那一级 `permission` 给调用写上沙盒（第 1a 条）：这台机器上的沙盒用不了的，照原样。在阻塞线程里算。
/// 写不成的不跑：记一行 `ERROR`，交回空的，照崩了算，不在沙盒外跑。`unless` 是管理员放行了的那件工具的文件（施工 X-37 补）：
/// 落在规格写得了的地方的照旧套，不在的不套。
pub(super) async fn confine(
    mut call: Call,
    sandbox: Option<Sandbox>,
    (permission, dirs, grants): (Permission, Vec<String>, Vec<RawJson>),
    (call_id, unless): (CallId, Option<PathBuf>),
) -> Option<Call> {
    let Some(sandbox) = sandbox else {
        return Some(call);
    };
    let cwd = call.cwd.clone();
    match blocking(move || sandbox.for_call(&permission, &cwd, &dirs, &grants)).await {
        Ok(sandboxed) => {
            let bare = unless.is_some_and(|real| {
                sandboxed.as_ref().is_some_and(|sandboxed| {
                    !sandboxed.spec.write.iter().any(|dir| real.starts_with(dir))
                })
            });
            call.sandbox = sandboxed.filter(|_| !bare).map(Arc::new);
            Some(call)
        }
        Err(error) => {
            tracing::error!(
                target: TARGET,
                call = call_id.to_string().as_str(),
                error = %error,
                "sandbox not set up"
            );
            None
        }
    }
}

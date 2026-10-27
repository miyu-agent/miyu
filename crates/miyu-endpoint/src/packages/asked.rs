//! `package.install`、`package.remove` 照参数、照层分（施工 F-5 上；施工 U23 下上从 `manage.rs` 挪出来、按 [`Scope`] 分）：
//! 系统那一层的照 `manage.rs`，自己那一层的照 `mine.rs`。装回出厂的、整组装卸只有系统那一层做得了。

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use super::scope::Scope;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `package.install` 的参数：从一份清单装（`path`），或者把卸掉的出厂的包装回来（`package`），二者写一个。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstallParams {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    package: Option<String>,
    /// 只看一眼、什么都不动（施工 F-8 下补，`preview.rs`）。
    #[serde(default)]
    preview: bool,
}

/// `package.remove` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveParams {
    package: String,
    /// 只看一眼、什么都不动（施工 F-8 下补）。
    #[serde(default)]
    preview: bool,
}

/// `package.install`，装进 `scope` 那一层。成员从本机路径装的只能是他自己家目录里的（[`super::mine::reachable`]）。
pub(crate) async fn install(
    core: &Arc<Core>,
    peer: Peer,
    scope: &Scope,
    params: InstallParams,
) -> Result<Value, Refusal> {
    let _one_at_a_time = core.packaging.lock().await;
    match (params.path, params.package, params.preview) {
        (Some(path), None, preview) => {
            let path = Path::new(&path);
            if let Scope::Own(account) = scope {
                super::mine::reachable(core, peer.admin, account, path)?;
            }
            match (super::archive::is_package_file(path), preview, scope) {
                (true, ..) => super::archive::install(core, peer, scope, path, preview).await,
                (false, true, _) => super::preview::installing(core, peer, scope, path).await,
                (false, false, Scope::System) => {
                    super::manage::from_path(core, peer, path, None).await
                }
                (false, false, Scope::Own(account)) => {
                    super::mine::from_path(core, peer, account, path, None).await
                }
            }
        }
        (None, Some(id), preview) => {
            scope.system_only(peer.admin)?;
            match preview {
                true => super::preview::restoring(core, peer, &id).await,
                false => super::manage::bring_back(core, peer, &id).await,
            }
        }
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// `package.remove`：卸 `scope` 那一层的。系统那一层的：家目录那一层的删掉，出厂的记一笔，被依赖着的拒绝。
pub(crate) async fn remove(
    core: &Arc<Core>,
    peer: Peer,
    scope: &Scope,
    params: RemoveParams,
) -> Result<Value, Refusal> {
    match scope {
        Scope::System if params.preview => {
            let _one_at_a_time = core.packaging.lock().await;
            super::preview::removing(core, &params.package).await
        }
        Scope::System => super::manage::remove_package(core, params.package).await,
        Scope::Own(account) => {
            let _one_at_a_time = core.packaging.lock().await;
            super::mine::remove(core, peer, account, params.package, params.preview).await
        }
    }
}

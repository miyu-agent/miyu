//! `package.install`、`package.remove` 照参数分（施工 F-5 上；施工 U23 下上从 `manage.rs` 挪出来）：包文件、包文件夹、装回出厂的、
//! 看一眼。

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::reverse::Peer as Reverse;

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

/// `package.install`。从包文件、包文件夹真装的，核对、装之前各推一次 `package.progress` 给发起的连接 `caller`（施工 IN-3）；
/// 她的工具经端口装的没有连接。
pub(crate) async fn install(
    core: &Arc<Core>,
    peer: Peer,
    caller: Option<&Reverse>,
    params: InstallParams,
) -> Result<Value, Refusal> {
    let _one_at_a_time = core.packaging.lock().await;
    match (params.path, params.package, params.preview) {
        (Some(path), None, preview) => {
            let path = Path::new(&path);
            match (super::archive::is_package_file(path), preview) {
                (true, _) => super::archive::install(core, peer, path, preview, caller).await,
                (false, true) => super::preview::installing(core, peer, path).await,
                (false, false) => super::manage::from_path(core, (peer, caller), path, None).await,
            }
        }
        (None, Some(id), preview) => match preview {
            true => super::preview::restoring(core, peer, &id).await,
            false => super::manage::bring_back(core, peer, &id).await,
        },
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// `package.remove`：装上的删掉，出厂的记一笔，被依赖着的拒绝。
pub(crate) async fn remove(core: &Arc<Core>, params: RemoveParams) -> Result<Value, Refusal> {
    if params.preview {
        let _one_at_a_time = core.packaging.lock().await;
        return super::preview::removing(core, &params.package).await;
    }
    super::manage::remove_package(core, params.package).await
}

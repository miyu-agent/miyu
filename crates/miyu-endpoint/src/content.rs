//! 给别的包带的内容（施工 X-22，`docs/blueprint/packages.md`「给别的包带的内容」）：清单写了 `[content.<包>] dir` 的包，给那个
//! 包带了一个目录的东西。被带的那个包的扩展（核心拉起的，`caller.package` 是它）经 `content.list` 看谁给它带了，经
//! `content.read` 分块读，只读得到那个目录里的；别的连接 `not_a_provider`。不要能力：东西是别的包写明了给它的。
//!
//! 核心每重读一次清单（装、卸、升级、装回出厂的包），给它带内容的包这前后有一个就推 `content.changed`（`{}`），扩展再
//! `content.list`、自己比变没变；一次装卸至多多推一回，不用轮询。

use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::Core;
use crate::backstage::{beneath, segments};
use crate::hello::Caller;
use crate::refusal::Refusal;

/// 核心重读清单的次数：每重读一次加一（`Core::reload_packages`）。
#[derive(Debug)]
pub(crate) struct Rereads(watch::Sender<u64>);

impl Default for Rereads {
    fn default() -> Rereads {
        Rereads(watch::channel(0).0)
    }
}

impl Rereads {
    /// 又重读了一次。
    pub(crate) fn bump(&self) {
        self.0.send_modify(|n| *n += 1);
    }
}

/// `content.list` 的参数：没有。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListParams {}

/// `content.read` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadParams {
    package: String,
    path: String,
    #[serde(default)]
    offset: u64,
}

/// 给包 `target` 带了内容的包，照编号排：编号、版本、内容在哪个目录。它自己给自己带的照样算（就是它自己的目录）。
fn providers(core: &Core, target: &str) -> Vec<(String, Option<String>, PathBuf)> {
    let mut found: Vec<_> = core
        .packages()
        .iter()
        .filter_map(|found| {
            let manifest = found.read.as_ref().ok()?;
            let content = manifest.content.iter().find(|one| one.target == target)?;
            Some((
                found.id.clone(),
                manifest.version.clone(),
                found.files_dir().join(&content.dir),
            ))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn own(caller: &Caller) -> Result<&str, Refusal> {
    caller.package.as_deref().ok_or(Refusal::NOT_A_PROVIDER)
}

/// `content.list`：`{"packages": [{"package", "version"?}]}`。
pub(crate) fn list(core: &Core, caller: &Caller, _: ListParams) -> Result<Value, Refusal> {
    let target = own(caller)?;
    let packages: Vec<Value> = providers(core, target)
        .into_iter()
        .map(|(package, version, _)| match version {
            Some(version) => json!({"package": package, "version": version}),
            None => json!({"package": package}),
        })
        .collect();
    Ok(json!({ "packages": packages }))
}

/// `content.read`：读一块，`{"data", "size", "eof"}`，同 `package.file`。没有这个包、它没给调的一方带内容的 `unknown_package`；
/// 路径空的、写错的 `bad_params`；没有这一份、链接指出目录的 `not_found`。
pub(crate) async fn read(
    core: &Core,
    caller: &Caller,
    params: ReadParams,
) -> Result<Value, Refusal> {
    let target = own(caller)?;
    if params.path.is_empty() {
        return Err(Refusal::BAD_PARAMS);
    }
    let parts = segments(&params.path).ok_or(Refusal::BAD_PARAMS)?;
    let (_, _, dir) = providers(core, target)
        .into_iter()
        .find(|(package, _, _)| *package == params.package)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let offset = params.offset;
    tokio::task::spawn_blocking(move || {
        let real = beneath(&dir, &parts).ok_or(Refusal::FILE_NOT_FOUND)?;
        let segment = miyu_fs::read_range(&real, offset, miyu_fs::MAX_LENGTH)
            .map_err(|_| Refusal::FILE_NOT_FOUND)?;
        let eof = offset.saturating_add(segment.data.len() as u64) >= segment.size;
        Ok(json!({"data": STANDARD.encode(&segment.data), "size": segment.size, "eof": eof}))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}

/// 盯着重读、推 `content.changed` 的任务：核心拉起的扩展握了手起，连接断了跟着停。
#[derive(Debug)]
pub(crate) struct Watch(JoinHandle<()>);

impl Watch {
    /// 包是 `package`，写进 `out`。
    pub(crate) fn start(core: Arc<Core>, package: String, out: mpsc::Sender<String>) -> Watch {
        // 先拿收重读的一头，再算一次：中间重读的照样收得到。
        let mut rereads = core.content_rereads.0.subscribe();
        Watch(tokio::spawn(async move {
            let mut had = !providers(&core, &package).is_empty();
            while rereads.changed().await.is_ok() {
                let has = !providers(&core, &package).is_empty();
                if had || has {
                    tracing::debug!(target: "miyu::endpoint", package = package.as_str(), "content changed");
                    let line = json!({"jsonrpc": "2.0", "method": "content.changed", "params": {}});
                    if out.send(line.to_string()).await.is_err() {
                        return;
                    }
                }
                had = has;
            }
        }))
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.0.abort();
    }
}

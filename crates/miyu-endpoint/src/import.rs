//! `session.import {blob | path, archive?}`（施工 SE-1 下，设计 `58-会话导出和导入.md` 第二、三节，`docs/blueprint/protocol.md`
//! 「`session.import`」）：把 `session.export` 打的原始包导进这个数据根。
//!
//! - 包从哪来：头先 `blob.put` 传上来交 `blob`；本机的头（命令行）直接给 `path`（绝对路径）。两样只写一样。
//! - 先核对再落盘（[`miyu_store::bundle::import`]）：坏了什么都不留；同编号、内容一样的当导过了跳过，不一样的拒。
//! - `archive: true` 的是只读存档（迁移程序导旧版的会话都这样）；包里写了存档的照样是存档。存档能翻、能搜，不能接着说
//!   （[`refused`]）。
//! - 导完：新落盘的会话在后台接进回合库，不等人打开一次才搜得到；会话列表推一条变化。
//! - 只给人的头（`connection/rights.rs`）：包是外面来的，导进来就是这个人的会话。

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{ContentHash, SessionId};
use miyu_store::blob::Blobs;
use miyu_store::bundle::{self, ImportError, Marker};
use miyu_store::root::DataRoot;

use crate::Core;
use crate::refusal::Refusal;

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// `session.import` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportParams {
    /// 先 `blob.put` 传上来的包。
    #[serde(default)]
    blob: Option<String>,
    /// 本机的包，绝对路径。
    #[serde(default)]
    path: Option<PathBuf>,
    /// 导成只读存档。
    #[serde(default)]
    archive: bool,
}

/// `session.import`：见模块的说明。回应 `{"main", "sessions", "present"}`：包的主会话、这一次新导进来的、早就在的。
pub(crate) async fn import(core: &Arc<Core>, params: ImportParams) -> Result<Value, Refusal> {
    let root = core.root.clone();
    let bundle = match (params.blob, params.path) {
        (Some(blob), None) => {
            let hash = ContentHash::parse(&blob).map_err(|_| Refusal::BAD_PARAMS)?;
            let path = Blobs::new(root.blobs()).path(&hash);
            if path.symlink_metadata().is_err() {
                return Err(Refusal::UNKNOWN_ATTACHMENT);
            }
            path
        }
        (None, Some(path)) if path.is_absolute() => path,
        _ => return Err(Refusal::BAD_PARAMS),
    };
    let marker = Marker {
        archive: params.archive,
        miyu: String::new(),
        exported: String::new(),
    };
    let done = tokio::task::spawn_blocking(move || {
        bundle::import(
            &bundle,
            &root.sessions_dir(),
            &Blobs::new(root.blobs()),
            &marker,
        )
        .map_err(refused_import)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    for session in &done.added {
        core.listing.changed(core, session.clone());
    }
    if !done.added.is_empty() {
        core.memory.index_imported(done.added.clone());
    }
    tracing::info!(
        target: TARGET,
        session = done.manifest.main.as_str(),
        added = done.added.len(),
        present = done.present.len(),
        archive = params.archive,
        "sessions imported"
    );
    let ids = |list: &[SessionId]| {
        list.iter()
            .map(|id| id.as_str().to_string())
            .collect::<Vec<_>>()
    };
    Ok(json!({
        "main": done.manifest.main,
        "sessions": ids(&done.added),
        "present": ids(&done.present),
    }))
}

/// 导入失败照哪一种拒。
fn refused_import(error: ImportError) -> Refusal {
    match error {
        ImportError::Bad(why) => {
            tracing::warn!(target: TARGET, error = why.as_str(), "bundle not read");
            Refusal::BAD_BUNDLE
        }
        ImportError::Mismatch(why) => {
            tracing::warn!(target: TARGET, error = why.as_str(), "bundle mismatch");
            Refusal::BUNDLE_MISMATCH
        }
        ImportError::TooNew(_) => Refusal::BUNDLE_TOO_NEW,
        ImportError::Exists(session) => Refusal::session_exists(session.as_str()),
        ImportError::Io(error) => {
            tracing::warn!(target: TARGET, error = %error, "bundle not imported");
            Refusal::INTERNAL
        }
    }
}

/// 这个会话是不是导进来的只读存档（[`bundle::archived`]）。
pub(crate) fn archived(root: &DataRoot, session: &SessionId) -> bool {
    bundle::archived(&root.session_dir(session))
}

/// 改会话的方法照到的是只读存档的，拒 `archived`（设计 58 第三节第 3 条）：说话、`!` 跑命令、打断、撤销、恢复、重做、压缩、
/// 清空、换配置、分叉、换权限级别和工作目录、回答、跑斜杠命令、停任务、包在会话里的状态、定时任务指到它。读的、删的、改标题
/// 的照常。没写会话、会话编号不合写法的交给方法自己报。
pub(crate) fn refused(core: &Core, method: &str, params: Option<&Value>) -> Option<Refusal> {
    const CHANGING: &[&str] = &[
        "session.send",
        "session.interrupt",
        "session.withdraw",
        "session.revert",
        "session.unrevert",
        "session.redo",
        "session.compact",
        "session.clear",
        "session.configure",
        "session.fork",
        "session.respond",
        "session.answer",
        "session.set_permission_level",
        "session.set_workspace",
        "session.set_state",
        "command.run",
        "job.stop",
    ];
    let params = params?;
    let session = if CHANGING.contains(&method) {
        params.get("session")
    } else if method == "schedule.add" {
        params
            .get("target")
            .and_then(|target| target.get("session"))
    } else {
        None
    }?;
    let session = SessionId::parse(session.as_str()?).ok()?;
    archived(&core.root, &session).then_some(Refusal::ARCHIVED)
}

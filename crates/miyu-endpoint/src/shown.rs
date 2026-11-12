//! `session.shown`（施工 SW-1，`docs/blueprint/protocol.md` 的 `session.shown`，`tools/show.md`「头怎么读内容」）：远程的头分段读
//! 她展示过的一个文件。展示的文件不存成 blob，只在 `file.shown` 里记真实路径、大小、修改时间；读不到这台电脑硬盘的头照「会话 +
//! 调用编号」来要，核心照原路径边读边传。
//!
//! 守的几条：
//! - 不收路径：只读那次展示记下的那一个（参数多写的格 `bad_params`）。
//! - 谁读得到同带 `session` 的 `blob.get`（[`crate::attach::reachable`]）；会话没有、读不到、找不到这次展示的一律
//!   `unknown_shown`，不说是哪一样。
//! - 读之前照打开的句柄核对大小和修改时间，对不上 `file_changed`，没了 `file_gone`：展示以后换掉的文件读不到。
//! - 找法不整份翻日志：调用编号里带着发起它的助手消息的序号，从那里读起、只解 `tool.result`，找到就停。视频拖进度一段段来读，
//!   每次只读那一小截。

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_fs::{OpenError, open_file};
use miyu_kernel::event::{Body, Effect, FileShown};
use miyu_kernel::id::{CallId, SessionId};
use miyu_store::log::{Flow, OpenError as LogError, read_range_only};
use miyu_tool::modified_at;

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// `session.shown` 的参数：`session`、`call` 必写；`offset`、`length` 同 `blob.get`。别的格不认（`bad_params`）：核心不收路径。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Params {
    session: String,
    /// 展示的那一次调用的编号：视图工具卡的 `call`。
    call: String,
    #[serde(default)]
    offset: u64,
    #[serde(default = "default_length")]
    length: u64,
}

/// 不写 `length` 时读多少：同 `blob.get`。
fn default_length() -> u64 {
    miyu_fs::MAX_LENGTH
}

/// `session.shown`：回应同 `blob.get`，`{"data", "size"}`；`caller` 是连接（平台桥照它的包认）。
///
/// # Errors
///
/// 写法不对、`length` 超过 512 KiB：`bad_params`；会话没有、读不到、日志里没有这次展示：`unknown_shown`；文件变了
/// `file_changed`、没了 `file_gone`；日志、文件读不了：`internal_error`。
pub(crate) async fn read(core: &Core, caller: &Caller, params: Params) -> Result<Value, Refusal> {
    if params.length > miyu_fs::MAX_LENGTH {
        return Err(Refusal::BAD_PARAMS);
    }
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let call = CallId::parse(&params.call).map_err(|_| Refusal::BAD_PARAMS)?;
    if !crate::attach::reachable(core, caller, &session).await {
        return Err(Refusal::UNKNOWN_SHOWN);
    }
    let dir = core.root.session_dir(&session);
    let (offset, length) = (params.offset, params.length);
    tokio::task::spawn_blocking(move || {
        let shown = find(&dir, call)?.ok_or(Refusal::UNKNOWN_SHOWN)?;
        piece(&shown, offset, length)
    })
    .await
    .unwrap_or_else(|error| {
        tracing::error!(target: TARGET, panic = error.is_panic(), "shown file read panicked");
        Err(Refusal::INTERNAL)
    })
}

/// 会话目录 `dir` 里调用 `call` 的结果记下的 `file.shown`：从发起它的助手消息读起，只解 `tool.result`，找到这次调用的结果就停。
/// 结果里没有 `file.shown` 的、读到最后都没有这次调用的结果的没有。
fn find(dir: &Path, call: CallId) -> Result<Option<FileShown>, Refusal> {
    let mut found = None;
    let read = read_range_only(dir, call.message(), None, &["tool.result"], |events| {
        let result = events.into_iter().find_map(|event| match event.body {
            Body::ToolResult(result) if result.call_id == call => Some(result),
            _ => None,
        });
        match result {
            Some(result) => {
                found = result.effects.into_iter().find_map(|effect| match effect {
                    Effect::FileShown(shown) => Some(shown),
                    _ => None,
                });
                Flow::Stop
            }
            None => Flow::Go,
        }
    });
    match read {
        Ok(()) => Ok(found),
        Err(LogError::Missing(_)) => Err(Refusal::UNKNOWN_SHOWN),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "shown file not found in log");
            Err(Refusal::INTERNAL)
        }
    }
}

/// 照记下的路径安全地打开，照打开的那个句柄核对大小和修改时间，对上了从 `offset` 读最多 `length` 个字节。
fn piece(shown: &FileShown, offset: u64, length: u64) -> Result<Value, Refusal> {
    let path = PathBuf::from(&shown.path);
    let mut file = match open_file(&path) {
        Ok(file) => file,
        Err(OpenError::NotFound | OpenError::NotAFile(_)) => return Err(Refusal::FILE_GONE),
        Err(OpenError::Io(error)) => return Err(failed(&error)),
    };
    let metadata = file.metadata().map_err(|error| failed(&error))?;
    // 以前（施工 SW-1 以前）记的没有修改时间，核对不了，也当变了。
    let same = metadata.len() == shown.size
        && shown.modified.is_some()
        && modified_at(&metadata) == shown.modified;
    if !same {
        return Err(Refusal::FILE_CHANGED);
    }
    let mut data = Vec::new();
    if length > 0 && offset < shown.size {
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.take(length).read_to_end(&mut data))
            .map_err(|error| failed(&error))?;
    }
    Ok(json!({"data": STANDARD.encode(&data), "size": shown.size}))
}

/// 打得开却读不了（没有权限之类）：记一行，`internal_error`。
fn failed(error: &std::io::Error) -> Refusal {
    tracing::warn!(target: TARGET, error = %error, "shown file not read");
    Refusal::INTERNAL
}

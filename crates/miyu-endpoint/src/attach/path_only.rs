//! 只有路径的附件（施工 AT-1，`docs/blueprint/protocol.md`「`session.send`」第 5 条）：超过上传上限的文件，头不 `blob.put`，
//! 只把它在核心那台机器上的路径交上来：`{"path", "name", "size"?}`，没有 `blob`。核心不读它、不查它在不在，只查写法，
//! 记成一块 `path`（[`PathOnly`]）；她要看，照路径用工具去读，读的时候照权限策略判。
//!
//! 只有 `session.send`、`session.redo` 收（[`Sent`]）；回答提问、拒绝时附的照旧要 `blob`（[`Attachment`]）。场所会话不收
//! （[`local_only`]）：平台桥代表平台上的人说话，那边的人指不了核心这台机器上的路径。

use serde::Deserialize;
use serde_json::Value;

use miyu_kernel::block::{Block, PathOnly};
use miyu_kernel::id::SourcePath;

use super::{Attachment, file_name};
use crate::refusal::Refusal;

/// `session.send`、`session.redo` 的一个附件：写了 `blob` 的是 `blob.put` 的回应（[`Attachment`]，一格都不变）；没写
/// `blob`、写了 `path` 的只有路径。读参数时就分开：有 `blob` 的缺 `name`、`media_type`，没有 `blob` 的缺 `path`、`name`，
/// 都当场读不进（`bad_params`），和原来缺格时一样早。
#[derive(Debug, Deserialize)]
#[serde(try_from = "Written")]
pub(crate) enum Sent {
    /// 存进了 blob 的。
    Stored(Attachment),
    /// 只有路径的。
    PathOnly(Pointed),
}

/// 头写的那几格，哪一种都先读成这样；`null` 等于没写。
#[derive(Debug, Deserialize)]
struct Written {
    #[serde(default)]
    blob: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(default)]
    path: Option<String>,
    /// 只有路径的才看；有 `blob` 的照旧不看这一格，写成什么都行。
    #[serde(default)]
    size: Option<Value>,
}

impl TryFrom<Written> for Sent {
    type Error = &'static str;

    fn try_from(written: Written) -> Result<Sent, Self::Error> {
        match (written.blob, written.name, written.media_type, written.path) {
            (Some(blob), Some(name), Some(media_type), path) => Ok(Sent::Stored(Attachment {
                blob,
                name,
                media_type,
                path,
            })),
            (Some(_), ..) => Err("an attachment with `blob` needs `name` and `media_type`"),
            (None, Some(name), _, Some(path)) => Ok(Sent::PathOnly(Pointed {
                path,
                name,
                size: written.size,
            })),
            (None, ..) => Err("an attachment needs `blob`, or `path` and `name`"),
        }
    }
}

/// 只有路径的一项，还没查写法。
#[derive(Debug)]
pub(crate) struct Pointed {
    path: String,
    name: String,
    size: Option<Value>,
}

impl Pointed {
    /// 查写法、造成块：`path` 照 [`SourcePath`]（绝对路径、没有控制字符、最多 4096 字节），`name` 照文件名的写法，
    /// `size` 是不带小数的非负整数。不碰磁盘。
    ///
    /// # Errors
    ///
    /// 有一格不合写法：`bad_params`。
    pub(super) fn block(self) -> Result<Block, Refusal> {
        let path = SourcePath::parse(&self.path).map_err(|_| Refusal::BAD_PARAMS)?;
        let name = file_name(&self.name)?;
        let size = self
            .size
            .map(|size| size.as_u64().ok_or(Refusal::BAD_PARAMS))
            .transpose()?;
        Ok(Block::Path(PathOnly { name, path, size }))
    }
}

/// 场所会话不收只有路径的附件：`local` 是不是本机的会话；不是的、`blocks` 里有这种块的，`bad_params`。
///
/// # Errors
///
/// 场所会话里带了只有路径的附件：`bad_params`。
pub(crate) fn local_only(blocks: &[Block], local: bool) -> Result<(), Refusal> {
    match !local && blocks.iter().any(|block| matches!(block, Block::Path(_))) {
        true => Err(Refusal::BAD_PARAMS),
        false => Ok(()),
    }
}

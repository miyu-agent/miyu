//! 没有本机路径的附件落成会话里的文件（施工 3-9 六补，`docs/blueprint/protocol.md`「`session.send`」第 5 条）：剪贴板贴的、
//! 截图、远程的头传内容的，块里没有 `path`；核心照会话的属主读 blob，存进会话目录的 `attachments/`
//! （[`miyu_store::attachments`]），把路径填回块里。记进日志的块带着路径，以后每次请求照它拼，字节不变。
//! 头交了路径的（本机文件）不拷：她读原文件，照权限判。

use miyu_kernel::block::Block;
use miyu_kernel::id::{AccountId, SessionId, SourcePath};
use miyu_store::attachments;
use miyu_store::blob::Blobs;

use super::{TARGET, blocking, place, read_blob};
use crate::Core;
use crate::refusal::Refusal;

/// 把 `blocks` 里没有路径的图片、文件存进会话 `session`（属主 `owner`）的附件目录，路径填回块里。blob 照属主读：调用前
/// 先 [`super::hand_over`]。读不了、存不下的记一行、`internal_error`。
pub(crate) async fn keep(
    core: &Core,
    owner: &AccountId,
    session: &SessionId,
    blocks: &mut Vec<Block>,
) -> Result<(), Refusal> {
    if !blocks.iter().any(pathless) {
        return Ok(());
    }
    let place = place(core);
    let dir = attachments::dir(&place.root, owner, session);
    let blobs = Blobs::new(place.root.blobs(owner));
    let taken = std::mem::take(blocks);
    *blocks = blocking(move || {
        taken
            .into_iter()
            .map(|block| kept(&blobs, &dir, block))
            .collect()
    })
    .await?;
    Ok(())
}

/// 图片、文件块，没有路径的。
fn pathless(block: &Block) -> bool {
    match block {
        Block::Image(image) => image.path.is_none(),
        Block::File(file) => file.path.is_none(),
        _ => false,
    }
}

/// 一块：没有路径的图片、文件存成文件、填上路径，别的原样。
fn kept(blobs: &Blobs, dir: &std::path::Path, block: Block) -> Result<Block, Refusal> {
    Ok(match block {
        Block::Image(mut image) if image.path.is_none() => {
            let name = image
                .name
                .as_ref()
                .map_or(attachments::clean(""), |name| name.as_str().to_string());
            image.path = Some(stored(
                blobs,
                dir,
                &image.blob,
                &name,
                image.media_type.as_str(),
            )?);
            Block::Image(image)
        }
        Block::File(mut file) if file.path.is_none() => {
            file.path = Some(stored(
                blobs,
                dir,
                &file.blob,
                file.name.as_str(),
                file.media_type.as_str(),
            )?);
            Block::File(file)
        }
        block => block,
    })
}

/// 读出 blob、存成文件，交回它的路径。
fn stored(
    blobs: &Blobs,
    dir: &std::path::Path,
    blob: &miyu_kernel::id::ContentHash,
    name: &str,
    media_type: &str,
) -> Result<SourcePath, Refusal> {
    let bytes = read_blob(blobs, blob)?;
    let path = attachments::keep(dir, name, media_type, &bytes).map_err(|error| {
        tracing::warn!(target: TARGET, blob = blob.as_str(), error = %error, "attachment not kept");
        Refusal::INTERNAL
    })?;
    SourcePath::parse(&path.to_string_lossy()).map_err(|_| {
        tracing::warn!(target: TARGET, path = %path.display(), "kept attachment path not usable");
        Refusal::INTERNAL
    })
}

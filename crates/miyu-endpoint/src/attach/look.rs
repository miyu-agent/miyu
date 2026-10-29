//! 读图（施工 8-20 `model.call` 的图、施工 P-5 人格的头像）：blob 读出来照内容认成图片。

use miyu_kernel::block::{Block, Image};
use miyu_kernel::id::{ContentHash, MediaType};
use miyu_store::blob::Blobs;
use miyu_tool::attachment::{Kind, kind};

use super::{blocking, place, read_blob};
use crate::Core;
use crate::refusal::Refusal;

/// `model.call` 的图（施工 8-20）：每个哈希一块图片，照先后，从唯一的存处读。
/// 没有的 `unknown_attachment`，不是图的 `bad_params`，太大的 `attachment_too_big`。
pub(crate) async fn images(core: &Core, hashes: Vec<ContentHash>) -> Result<Vec<Block>, Refusal> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let place = place(core);
    blocking(move || {
        let blobs = Blobs::new(place.root.blobs());
        hashes
            .into_iter()
            .map(|blob| {
                let bytes = read_blob(&blobs, &blob)?;
                match kind(&bytes, None).map_err(|_| Refusal::ATTACHMENT_TOO_BIG)? {
                    Kind::Image {
                        media_type,
                        width,
                        height,
                    } => Ok(Block::Image(Image {
                        blob,
                        name: None,
                        media_type,
                        width,
                        height,
                        path: None,
                    })),
                    Kind::File { .. } => Err(Refusal::BAD_PARAMS),
                }
            })
            .collect()
    })
    .await
}

/// 一个 blob 读出来、照内容认（施工 P-5：人格的头像）：交回字节和认出来的图；不是图、超了图的上限的
/// 交回 `None` 的图。blob 没有的 `unknown_attachment`。
pub(crate) async fn picture(
    core: &Core,
    blob: ContentHash,
) -> Result<(Vec<u8>, Option<(MediaType, u32, u32)>), Refusal> {
    let place = place(core);
    blocking(move || {
        let bytes = read_blob(&Blobs::new(place.root.blobs()), &blob)?;
        let image = match kind(&bytes, None) {
            Ok(Kind::Image {
                media_type,
                width,
                height,
            }) => Some((media_type, width, height)),
            Ok(Kind::File { .. }) | Err(_) => None,
        };
        Ok((bytes, image))
    })
    .await
}

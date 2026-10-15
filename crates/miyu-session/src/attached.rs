//! 她带的附件存成 blob、造成块（施工 C-9，`docs/blueprint/tools/send_message.md`「附件」）：`send_message` 的话、`subagent`
//! 的交代都经它。存在发话这个会话的属主名下：她发得到的别的会话、她派的子会话都是同一个属主，读得到。块的样子同人附的
//! （`docs/blueprint/protocol.md` 的 `session.send` 第 4 条）：图片带名字，两种都带原来的路径。

use miyu_kernel::block::{Block, File, Image, Text};
use miyu_store::blob::Blobs;
use miyu_tool::attachment::{Attached, Kind};

use crate::blocking::blocking;

/// 字一块在前，附件照先后接在后面。没带附件的只有字那一块，不碰磁盘。有一个存不下的（磁盘满了之类）交回系统说的：
/// 整句不发。
pub(crate) async fn blocks(
    blobs: &Blobs,
    text: &str,
    attached: Vec<Attached>,
) -> Result<Vec<Block>, String> {
    let said = Block::Text(Text {
        text: text.to_string(),
    });
    if attached.is_empty() {
        return Ok(vec![said]);
    }
    let blobs = blobs.clone();
    blocking(move || {
        let mut blocks = vec![said];
        for one in attached {
            let blob = blobs.put(&one.bytes).map_err(|error| error.to_string())?;
            blocks.push(match one.kind {
                Kind::Image {
                    media_type,
                    width,
                    height,
                } => Block::Image(Image {
                    blob,
                    name: Some(one.name),
                    media_type,
                    width,
                    height,
                    path: one.path,
                }),
                Kind::File { media_type } => Block::File(File {
                    blob,
                    name: one.name,
                    media_type,
                    path: one.path,
                }),
            });
        }
        Ok(blocks)
    })
    .await
}

#[cfg(test)]
mod tests;

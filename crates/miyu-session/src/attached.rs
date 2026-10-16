//! 她带的附件存成 blob、造成块（施工 C-9，`docs/blueprint/tools/send_message.md`「附件」）：`send_message` 的话、`subagent`
//! 的交代都经它。存在发话这个会话的属主名下：她发得到的别的会话、她派的子会话都是同一个属主，读得到。块的样子同人附的
//! （`docs/blueprint/protocol.md` 的 `session.send` 第 4 条）：图片带名字，两种都带原来的路径。

use std::path::Path;

use miyu_kernel::block::{Block, File, Image, Text};
use miyu_kernel::id::{AccountId, SessionId, SourcePath};
use miyu_store::attachments;
use miyu_store::blob::Blobs;
use miyu_store::root::DataRoot;
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

/// 收的那个会话 `session`（属主 `owner`）读得到带过去的附件（施工 3-9 六补）：没有路径的、路径落在数据根 `root` 里的（发话
/// 那个会话的附件目录），照 blob 拷进收的会话自己的附件目录，路径换成那里的；本机别处的原样（她照权限读原文件）。拷不下的
/// 交回系统说的：整句不发。
pub(crate) async fn kept_for(
    root: &DataRoot,
    owner: &AccountId,
    session: &SessionId,
    blobs: &Blobs,
    blocks: Vec<Block>,
) -> Result<Vec<Block>, String> {
    let inside = std::fs::canonicalize(root.path()).unwrap_or_else(|_| root.path().to_path_buf());
    if !blocks.iter().any(|block| foreign(block, &inside)) {
        return Ok(blocks);
    }
    let dir = attachments::dir(root, owner, session);
    let blobs = blobs.clone();
    blocking(move || {
        blocks
            .into_iter()
            .map(|block| {
                if !foreign(&block, &inside) {
                    return Ok(block);
                }
                Ok(match block {
                    Block::Image(mut image) => {
                        let name = image.name.as_ref().map_or_else(
                            || attachments::clean(""),
                            |name| name.as_str().to_string(),
                        );
                        image.path = Some(kept(
                            &blobs,
                            &dir,
                            &image.blob,
                            &name,
                            image.media_type.as_str(),
                        )?);
                        Block::Image(image)
                    }
                    Block::File(mut file) => {
                        file.path = Some(kept(
                            &blobs,
                            &dir,
                            &file.blob,
                            file.name.as_str(),
                            file.media_type.as_str(),
                        )?);
                        Block::File(file)
                    }
                    block => block,
                })
            })
            .collect()
    })
    .await
}

/// 图片、文件块，没有路径的，或者路径落在数据根 `inside`（真实的位置）里的。
fn foreign(block: &Block, inside: &Path) -> bool {
    let path = match block {
        Block::Image(image) => &image.path,
        Block::File(file) => &file.path,
        _ => return false,
    };
    path.as_ref()
        .is_none_or(|path| miyu_fs::within(Path::new(path.as_str()), inside))
}

/// 把 blob 存成附件目录 `dir` 里的文件，交回路径。
fn kept(
    blobs: &Blobs,
    dir: &Path,
    blob: &miyu_kernel::id::ContentHash,
    name: &str,
    media_type: &str,
) -> Result<SourcePath, String> {
    let bytes = blobs.get(blob).map_err(|error| error.to_string())?;
    let path =
        attachments::keep(dir, name, media_type, &bytes).map_err(|error| error.to_string())?;
    SourcePath::parse(&path.to_string_lossy()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;

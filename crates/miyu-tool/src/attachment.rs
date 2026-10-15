//! 认一个附件是什么（`docs/blueprint/protocol.md` 的 `blob.put` 第 4 条）：照内容，不看扩展名。`blob.put` 存之前认一遍，
//! `session.send` 造块之前照同一份代码再认一遍：进到会话里的块，宽高、媒体类型都是核心自己量的。她发给别的会话、交代
//! 子代理时带的附件（施工 C-9，`docs/blueprint/tools/send_message.md`）也照它认：施工 C-9 从协议端点挪到这里共用。
//!
//! - 四种图之一、量得出宽高的是图片，媒体类型照认出的；超了上限的不收（`miyu_tool::picture`，和 `read` 同一套）。
//! - 别的都是文件。开头是 `%PDF-` 的是 PDF。
//! - 别的文件，头写了媒体类型的照写的，只是写成 PDF、图片的不算：驱动照媒体类型把 PDF 当 PDF 发、照块的种类把图当图
//!   发，内容不是却写成了，供应商会拒，这个会话以后的请求都跟着失败。没写、不算的，文本是 `text/plain`（和驱动认
//!   文本文件是同一条，`miyu_drivers::text_file`），别的 `application/octet-stream`。

use miyu_drivers::text_file;
use miyu_kernel::id::{FileName, MediaType, SourcePath};

use crate::picture;

/// 一个附件最多几个字节：20 MiB（`04-核心协议.md` 第十一节）。`blob.put`、分块上传的 `size`、她带的附件（施工 C-9）都照它。
pub const LIMIT: u64 = 20 * 1024 * 1024;

/// 她带的一个附件（施工 C-9）：工具照她的权限读好、认好，交给端口存成 blob、造成块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    /// 文件名：真实位置的最后一段。
    pub name: FileName,
    /// 原来在本机的哪儿：真实的位置，模型看不了这个附件时占位那一句带上它。写不成路径的写法（太长、有控制字符）的没有。
    pub path: Option<SourcePath>,
    /// 内容。
    pub bytes: Vec<u8>,
    /// 认出来的。
    pub kind: Kind,
}

/// 认出来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// 图片：媒体类型、宽、高，都是量出来的。
    Image {
        /// 例如 `image/png`。
        media_type: MediaType,
        /// 宽，像素。
        width: u32,
        /// 高，像素。
        height: u32,
    },
    /// 文件。
    File {
        /// 媒体类型。
        media_type: MediaType,
    },
}

/// 图片超了上限：超过 5 MiB，或者哪一边超过 8000 像素。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooBig;

/// PDF 的媒体类型：驱动只认这一种写法（`drivers/openai-chat.md` 第 9 条）。
const PDF: &str = "application/pdf";

/// 认 `bytes` 是什么；`given` 是头写的媒体类型。
///
/// # Errors
///
/// 是图片、却超了上限。
pub fn kind(bytes: &[u8], given: Option<MediaType>) -> Result<Kind, TooBig> {
    let head = &bytes[..bytes.len().min(picture::HEAD)];
    if let Some(found) = picture::kind(head)
        && let Some((width, height)) = picture::measure(bytes)
    {
        if !picture::fits(bytes.len() as u64, width, height) {
            return Err(TooBig);
        }
        return Ok(Kind::Image {
            media_type: media(found),
            width,
            height,
        });
    }
    if bytes.starts_with(b"%PDF-") {
        return Ok(Kind::File {
            media_type: media(PDF),
        });
    }
    let named = given.filter(|given| {
        let given = given.as_str();
        given != PDF && !given.starts_with("image/")
    });
    let media_type = named.unwrap_or_else(|| match text_file::is_text(bytes) {
        true => media("text/plain"),
        false => media("application/octet-stream"),
    });
    Ok(Kind::File { media_type })
}

/// 代码里写死的几种媒体类型，都合写法。
fn media(text: &str) -> MediaType {
    MediaType::parse(text).expect("写死的媒体类型合写法")
}

#[cfg(test)]
mod tests;

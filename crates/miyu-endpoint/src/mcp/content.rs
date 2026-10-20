//! 服务回来的内容换成结果块（施工 X-3 上，`mcp.md`「怎么走」第四条第 3、4 款）：文字照原文；图照 `read` 认的四种、量得出
//! 尺寸、不超上限的交回成图（执行器存成 blob、接在块后面），别的写一行说没给她；声音、二进制的资源同样写一行；链接写成
//! `[名字](地址)`；带文字的资源照文字；认不出的照紧凑的 JSON；一段文字都没有、有结构化结果的照紧凑的 JSON。文字加起来超过
//! [`LIMIT`] 个字符的留前面那么多，末尾写一行截掉了多少。结构化结果不管有没有文字，都原样交给脚本（`Done.data`，施工 M-2，
//! 设计 `39-脚本调工具.md` 第六节第 3 条：原来有文字时它被丢掉）；出错的不交。

use base64::Engine as _;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::MediaType;
use miyu_mcp::{Called, Content};
use miyu_tool::{Done, Picture, picture};

use super::tool::{Texts, render};

/// 文字最多留几个字符（同 Claude Code 给 MCP 结果设的上限：浏览器这类服务一次能回几万 token）。
pub(super) const LIMIT: usize = 100_000;

/// 没写类型的二进制资源算什么类型。
const OCTETS: &str = "application/octet-stream";

/// 一次调用回来的 `called` 换成结局：`isError` 的算出错。
pub(super) fn done(called: Called, texts: &Texts) -> Done {
    let mut pieces: Vec<String> = Vec::new();
    let mut images = Vec::new();
    let mut texted = false;
    for item in called.content {
        match item {
            Content::Text(text) => {
                texted = true;
                pieces.push(text);
            }
            Content::Image { data, mime } => match image(&data) {
                Some(picture) => images.push(picture),
                None => pieces.push(not_shown(texts, &mime, &data)),
            },
            Content::Audio { data, mime } => pieces.push(not_shown(texts, &mime, &data)),
            Content::Link { uri, name } => pieces.push(match name {
                Some(name) => format!("[{name}]({uri})"),
                None => uri,
            }),
            Content::Resource {
                uri,
                text,
                blob,
                mime,
            } => match (text, blob) {
                (Some(text), _) => {
                    texted = true;
                    pieces.push(text);
                }
                (None, Some(blob)) => {
                    pieces.push(not_shown(texts, mime.as_deref().unwrap_or(OCTETS), &blob));
                }
                (None, None) => pieces.push(uri),
            },
            Content::Other(raw) => pieces.push(raw.to_string()),
        }
    }
    if !texted && let Some(structured) = &called.structured {
        pieces.push(structured.to_string());
    }
    let data = called.structured.filter(|_| !called.is_error);
    let pieces = cut(pieces, texts);
    let blocks = match (pieces.is_empty(), images.is_empty()) {
        (true, true) => vec![Block::Text(Text {
            text: String::new(),
        })],
        _ => pieces
            .into_iter()
            .map(|text| Block::Text(Text { text }))
            .collect(),
    };
    Done {
        error: called.is_error,
        blocks,
        images,
        data,
        ..Done::ok("")
    }
}

/// base64 的 `data` 认成一张图：解得开、是 `read` 认的四种、量得出尺寸、不超上限的才算。
fn image(data: &str) -> Option<Picture> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    let head = &bytes[..bytes.len().min(picture::HEAD)];
    let media_type = MediaType::parse(picture::kind(head)?).ok()?;
    let (width, height) = picture::measure(&bytes)?;
    if !picture::fits(bytes.len() as u64, width, height) {
        return None;
    }
    Some(Picture {
        bytes,
        media_type,
        width,
        height,
    })
}

/// 不给她看的一样东西：类型 `mime`，base64 的 `data` 解出来多少字节（照长度算，不解）。
fn not_shown(texts: &Texts, mime: &str, data: &str) -> String {
    let bytes = (data.trim_end_matches('=').len() * 3 / 4).to_string();
    render(&texts.not_shown, &[("mime", mime), ("bytes", &bytes)])
        .trim_end()
        .to_string()
}

/// 文字加起来超过 [`LIMIT`] 个字符的，留前面那么多（截在中间的那一段截短，后面的不要），末尾加一段说截掉了多少。
fn cut(pieces: Vec<String>, texts: &Texts) -> Vec<String> {
    let total: usize = pieces.iter().map(|piece| piece.chars().count()).sum();
    if total <= LIMIT {
        return pieces;
    }
    let mut left = LIMIT;
    let mut kept = Vec::new();
    for piece in pieces {
        if left == 0 {
            break;
        }
        match piece.char_indices().nth(left) {
            Some((at, _)) => {
                kept.push(piece[..at].to_string());
                left = 0;
            }
            None => {
                left -= piece.chars().count();
                kept.push(piece);
            }
        }
    }
    let cut = (total - LIMIT).to_string();
    kept.push(render(&texts.cut, &[("cut", &cut)]).trim_end().to_string());
    kept
}

#[cfg(test)]
mod tests;

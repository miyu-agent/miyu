//! 超了上限的图缩到上限以内（施工 I-S，2026-10-22 项目主人排：超上限缩到上限，不报错）。她读到的、脚本交回的、人附的图都
//! 照这一份：每边不超过 [`MAX_SIDE`]、字节不超过 [`MAX_BYTES`]。
//!
//! - 解码有上限（[`DECODE_LIMIT`]），太大解不开的照旧当超了上限：一张几亿像素的图不值得吃掉几个 GiB 内存。
//! - 先照长边缩到 [`MAX_SIDE`]；编出来还超字节的，每次再缩到四分之三，最多几次。
//! - 有透明的编成 PNG，没有的编成 JPEG（质量 85）：照片编 PNG 常常还是超字节。动图只留第一帧。

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, Limits};

use super::{MAX_BYTES, MAX_SIDE};

#[cfg(test)]
mod tests;

/// 解码时最多分配多少字节：512 MiB，够一张 8000×8000 的 RGBA 还有富余。
pub const DECODE_LIMIT: u64 = 512 * 1024 * 1024;

/// 超了字节数以后每次再缩到原来的几分之几，最多缩几次。
const STEP: (u32, u32) = (3, 4);
const TRIES: usize = 8;

/// 缩好的一张图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shrunk {
    /// 编好的内容。
    pub bytes: Vec<u8>,
    /// `image/png` 或 `image/jpeg`。
    pub media_type: &'static str,
    /// 缩完的宽。
    pub width: u32,
    /// 缩完的高。
    pub height: u32,
}

/// 把 `bytes` 这张图缩到上限以内。解不开的（不是这四种图、坏了、太大解不开）、缩到最小还超的是空的。没超上限的原样交回
/// 不在这里管：调的人先照 [`super::fits`] 看。
pub fn shrink(bytes: &[u8]) -> Option<Shrunk> {
    within(bytes, MAX_BYTES, MAX_SIDE)
}

/// 同 [`shrink`]，上限照给的：每边不超过 `side`、字节不超过 `limit`。
fn within(bytes: &[u8], limit: u64, side: u32) -> Option<Shrunk> {
    let image = decode(bytes)?;
    let alpha = image.color().has_alpha();
    let (mut width, mut height) = bounded(image.width(), image.height(), side);
    for _ in 0..TRIES {
        let resized = image.resize_exact(width, height, FilterType::Triangle);
        let (bytes, media_type) = encode(&resized, alpha)?;
        if bytes.len() as u64 <= limit {
            return Some(Shrunk {
                bytes,
                media_type,
                width: resized.width(),
                height: resized.height(),
            });
        }
        width = (width * STEP.0 / STEP.1).max(1);
        height = (height * STEP.0 / STEP.1).max(1);
    }
    None
}

/// 照内容认格式、解码，分配有上限。
fn decode(bytes: &[u8]) -> Option<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(DECODE_LIMIT);
    reader.limits(limits);
    reader.decode().ok()
}

/// 宽 `width`、高 `height` 照比例缩到长边不超过 `side`；本来就不超的原样。
fn bounded(width: u32, height: u32, side: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= side {
        return (width, height);
    }
    let scale = |n: u32| ((u64::from(n) * u64::from(side) / u64::from(long)) as u32).max(1);
    (scale(width), scale(height))
}

/// 有透明的编 PNG，没有的编 JPEG。
fn encode(image: &DynamicImage, alpha: bool) -> Option<(Vec<u8>, &'static str)> {
    let mut out = Cursor::new(Vec::new());
    if alpha {
        image.write_to(&mut out, ImageFormat::Png).ok()?;
        return Some((out.into_inner(), "image/png"));
    }
    let rgb = DynamicImage::ImageRgb8(image.to_rgb8());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
    rgb.write_with_encoder(encoder).ok()?;
    Some((out.into_inner(), "image/jpeg"))
}

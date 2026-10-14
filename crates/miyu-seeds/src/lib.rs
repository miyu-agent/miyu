//! 从一张图挑主题色的候选（施工 P-6 补，`docs/blueprint/personas.md`「主题色、背景图」第 4 条）：照 MD3 的做法缩图、量化、打分，
//! 最多挑 4 个，打分最高的排第一，和 Android 选壁纸色一样。
//!
//! 核心拿它算人格头像的候选（`persona.list`、`persona.get` 的 `seeds`）；两个头经门面 `miyu-client` 拿它算壁纸的候选。
//! 2026-10-14 项目主人定：头像归核心，壁纸归头。同一份解码、缩图、量化、打分，同一张图在哪里挑出来的都一样。
//! 网页原来用的 JS 版量化里有随机数，同一张图每次挑的会抖，和 Rust 版也对不上，所以一律在这里算。

use material_colors::color::Rgb;
use material_colors::quantize::{Quantizer, QuantizerCelebi};
use material_colors::score::Score;

#[cfg(test)]
mod tests;

/// 最多挑几个候选：和 Android 12 选壁纸色时列出来的一样多。
pub const MAX: usize = 4;

/// 量化成最多几种颜色（MD3 取壁纸色的做法）。
const QUANTIZE: usize = 128;

/// 太透明的像素不算（头像圆角外面、抠了图的背景）。
const MIN_ALPHA: u8 = 200;

/// 挑之前先缩到边长不超过这么多：缩小了挑出来的一样，快得多。终端原来取头像主色就是这个大小。
const SIDE: u32 = 128;

/// 一个候选色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seed {
    /// 红。
    pub red: u8,
    /// 绿。
    pub green: u8,
    /// 蓝。
    pub blue: u8,
}

impl Seed {
    /// 写成小写的 `#rrggbb`：协议里、人格文件里都是这个写法。
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }

    /// 写成不透明的 ARGB：MD3 的库都收这个。
    pub fn argb(self) -> u32 {
        0xff00_0000 | u32::from(self.red) << 16 | u32::from(self.green) << 8 | u32::from(self.blue)
    }
}

impl From<Rgb> for Seed {
    fn from(rgb: Rgb) -> Self {
        Seed {
            red: rgb.red,
            green: rgb.green,
            blue: rgb.blue,
        }
    }
}

/// RGBA 像素（一格四个数）挑候选：太透明的不算，一格都不剩的是空的。打分照 MD3 的默认：最多 [`MAX`] 个，太灰、太少见的
/// 滤掉，一个都不合格的给 Google 蓝 `#4285f4`。
pub fn from_rgba(rgba: &[u8]) -> Vec<Seed> {
    let pixels: Vec<Rgb> = rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[3] >= MIN_ALPHA)
        .map(|pixel| Rgb::new(pixel[0], pixel[1], pixel[2]))
        .collect();
    if pixels.is_empty() {
        return Vec::new();
    }
    let quantized = QuantizerCelebi::quantize(&pixels, QUANTIZE);
    Score::score(&quantized.color_to_count, Some(MAX), None, None)
        .into_iter()
        .map(Seed::from)
        .collect()
}

/// 一张图的字节（PNG、JPEG、WebP、GIF、BMP，照内容认）挑候选：先照 `image` 的 `thumbnail` 等比缩到边长 128 再挑，和终端原来
/// 取头像主色的缩法一样（同一张头像挪过来以后颜色不变）。读不成的是 `None`。
pub fn from_image(bytes: &[u8]) -> Option<Vec<Seed>> {
    let image = image::load_from_memory(bytes).ok()?;
    Some(from_rgba(image.thumbnail(SIDE, SIDE).to_rgba8().as_raw()))
}

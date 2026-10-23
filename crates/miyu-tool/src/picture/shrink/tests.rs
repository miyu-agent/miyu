//! 缩图的测试：太宽的缩到长边上限、照比例；没透明的编成 JPEG、有透明的 PNG；字节超的再缩到不超；解不开的没有。

use std::io::Cursor;

use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

use super::*;
use crate::picture::{fits, measure};

fn png(image: DynamicImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn a_too_wide_picture_comes_down_to_the_side_limit() {
    let wide = png(DynamicImage::ImageRgb8(RgbImage::from_pixel(
        9000,
        30,
        Rgb([200, 100, 50]),
    )));
    let shrunk = shrink(&wide).expect("缩得了");
    assert_eq!((shrunk.width, shrunk.height), (8000, 26));
    assert_eq!(shrunk.media_type, "image/jpeg", "没透明的编 JPEG");
    assert_eq!(
        measure(&shrunk.bytes),
        Some((8000, 26)),
        "量出来的和说的一样"
    );
    assert!(fits(shrunk.bytes.len() as u64, shrunk.width, shrunk.height));
}

#[test]
fn transparency_stays_png() {
    let clear = png(DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        8100,
        81,
        Rgba([0, 0, 0, 0]),
    )));
    let shrunk = shrink(&clear).expect("缩得了");
    assert_eq!(shrunk.media_type, "image/png");
    assert_eq!((shrunk.width, shrunk.height), (8000, 80));
}

#[test]
fn too_many_bytes_shrinks_again_until_it_fits() {
    // 噪点几乎不压缩：240×240 的编出来几十 KiB；上限给 20 KiB、边给 1000，边没超，第一次照原尺寸编还超，要再缩几次。
    let mut seed = 7u32;
    let noisy = RgbImage::from_fn(240, 240, |_, _| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let [a, b, c, _] = seed.to_le_bytes();
        Rgb([a, b, c])
    });
    let bytes = png(DynamicImage::ImageRgb8(noisy));
    let limit = 20 * 1024;
    let shrunk = within(&bytes, limit, 1000).expect("缩得了");
    assert!(shrunk.bytes.len() as u64 <= limit);
    assert!(shrunk.width < 240, "缩过不止一次：{}", shrunk.width);
    assert_eq!(shrunk.width, shrunk.height, "照比例");
}

#[test]
fn what_does_not_decode_is_not_shrunk() {
    assert_eq!(shrink(b"\x89PNG\r\n\x1a\nnot really"), None);
    assert_eq!(shrink(b"hello"), None);
}

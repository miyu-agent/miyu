use std::io::Cursor;

use image::{ImageFormat, Rgba, RgbaImage};

use super::{MAX, Seed, from_image, from_rgba};

/// 一张 `width` × `height` 的图，左边 `left` 列是 `a`，右边是 `b`。
fn split(width: u32, height: u32, left: u32, a: [u8; 4], b: [u8; 4]) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, _| Rgba(if x < left { a } else { b }))
}

/// 图写成 PNG 的字节。
fn png(image: &RgbaImage) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    bytes.into_inner()
}

/// 色相：判断候选是不是偏红、偏蓝。
fn hue(seed: Seed) -> f64 {
    let hct: material_colors::hct::Hct =
        material_colors::color::Rgb::new(seed.red, seed.green, seed.blue).into();
    hct.get_hue()
}

#[test]
fn the_most_seen_color_comes_first_and_there_are_at_most_four() {
    let image = split(40, 40, 28, [200, 30, 40, 255], [30, 60, 210, 255]);
    let seeds = from_rgba(image.as_raw());
    assert!(!seeds.is_empty() && seeds.len() <= MAX, "{seeds:?}");
    assert!(
        hue(seeds[0]) < 40.0 || hue(seeds[0]) > 340.0,
        "红的多，排第一：{seeds:?}"
    );
    assert!(
        seeds
            .iter()
            .any(|seed| (230.0..300.0).contains(&hue(*seed))),
        "蓝的也是候选：{seeds:?}"
    );
}

#[test]
fn see_through_pixels_do_not_count() {
    let image = split(40, 40, 30, [20, 200, 60, 10], [210, 40, 40, 255]);
    let seeds = from_rgba(image.as_raw());
    assert!(
        hue(seeds[0]) < 40.0 || hue(seeds[0]) > 340.0,
        "透明的绿不算：{seeds:?}"
    );
    let clear = split(8, 8, 8, [20, 200, 60, 0], [0, 0, 0, 0]);
    assert!(from_rgba(clear.as_raw()).is_empty(), "全透明的没有候选");
}

#[test]
fn a_gray_picture_falls_back_to_google_blue() {
    let image = split(20, 20, 10, [120, 120, 120, 255], [200, 200, 200, 255]);
    let seeds = from_rgba(image.as_raw());
    assert_eq!(
        seeds,
        vec![Seed {
            red: 0x42,
            green: 0x85,
            blue: 0xf4
        }]
    );
    assert_eq!(seeds[0].hex(), "#4285f4");
    assert_eq!(seeds[0].argb(), 0xff42_85f4);
}

#[test]
fn image_bytes_are_shrunk_then_picked_and_the_same_picture_gives_the_same_seeds() {
    let big = split(600, 300, 400, [200, 30, 40, 255], [30, 60, 210, 255]);
    let seeds = from_image(&png(&big)).unwrap();
    assert!(hue(seeds[0]) < 40.0 || hue(seeds[0]) > 340.0, "{seeds:?}");
    assert_eq!(from_image(&png(&big)).unwrap(), seeds, "同一张图挑的一样");
    let small = image::DynamicImage::ImageRgba8(big).thumbnail(128, 128);
    assert_eq!(
        seeds,
        from_rgba(small.to_rgba8().as_raw()),
        "照 thumbnail 缩到 128 再挑"
    );
    assert_eq!(from_image(b"not a picture"), None);
}

#[test]
fn hex_is_lower_case_with_a_hash() {
    let seed = Seed {
        red: 0xAB,
        green: 0x0c,
        blue: 0xFF,
    };
    assert_eq!(seed.hex(), "#ab0cff");
}

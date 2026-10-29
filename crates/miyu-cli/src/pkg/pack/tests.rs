//! 打包时的图标（施工 F-18 上）：在上限以内的照原样；超了的缩到上限以内打进去，源文件不动，印一句；不在、不是图的打包失败。

use std::io::Cursor;
use std::path::PathBuf;

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

use super::*;

/// 一张 `width`×`height` 半透明的 PNG。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        width,
        height,
        Rgba([200, 80, 40, 160]),
    ))
    .write_to(&mut out, ImageFormat::Png)
    .expect("编得了");
    out.into_inner()
}

/// 这一次测试自己的目录：`name` 区分，旧的删掉。
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("miyu-cli-pack-{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("删得掉");
    }
    std::fs::create_dir_all(dir.join("kit/art")).expect("建得了");
    std::fs::write(
        dir.join("kit/package.toml"),
        "[package]\nversion = \"1.0\"\nname = { en = \"Kit\" }\nlogo = \"art/logo.png\"\n",
    )
    .expect("写得进");
    dir
}

/// 打一次：退出码、印出来的、报错的。
fn run(dir: &Path) -> (u8, String, String) {
    let wanted = Wanted {
        folder: "kit",
        target: None,
        out: Some("out"),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = pack(
        &wanted,
        dir,
        &Language::English,
        false,
        (&mut out, &mut err),
    );
    (
        code,
        String::from_utf8(out).expect("是字"),
        String::from_utf8(err).expect("是字"),
    )
}

/// 打好的包里的图标。
fn packed_logo(dir: &Path) -> Vec<u8> {
    let file = dir.join("out").join(archive::file_name("kit", "1.0", None));
    let unpacked = archive::unpack(&file, &dir.join("unpacked")).expect("解得开");
    std::fs::read(unpacked.folder.join("art/logo.png")).expect("在")
}

#[test]
fn a_logo_over_the_limits_is_shrunk_into_the_package_only() {
    let dir = scratch("big");
    let big = png(900, 600);
    std::fs::write(dir.join("kit/art/logo.png"), &big).expect("写得进");
    let (code, out, err) = run(&dir);
    assert_eq!(code, exit::OK, "{err}");
    assert!(
        out.starts_with("shrank logo art/logo.png to 512×341,"),
        "{out}"
    );
    let inside = packed_logo(&dir);
    assert_eq!(
        miyu_tool::picture::kind(&inside),
        Some("image/png"),
        "PNG 照 PNG"
    );
    assert_eq!(miyu_tool::picture::measure(&inside), Some((512, 341)));
    assert_eq!(
        std::fs::read(dir.join("kit/art/logo.png")).expect("在"),
        big,
        "源文件不动"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

#[test]
fn a_logo_within_the_limits_is_packed_as_it_is() {
    let dir = scratch("small");
    let small = png(64, 64);
    std::fs::write(dir.join("kit/art/logo.png"), &small).expect("写得进");
    let (code, out, err) = run(&dir);
    assert_eq!(code, exit::OK, "{err}");
    assert!(!out.contains("shrank"), "没缩不说：{out}");
    assert_eq!(packed_logo(&dir), small);
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

#[test]
fn a_missing_or_broken_logo_stops_the_packing() {
    let dir = scratch("missing");
    let (code, _, err) = run(&dir);
    assert_eq!(code, exit::ERROR);
    assert_eq!(err.trim(), "miyu: cannot pack: logo art/logo.png not found");
    std::fs::write(dir.join("kit/art/logo.png"), b"not a picture").expect("写得进");
    let (code, _, err) = run(&dir);
    assert_eq!(code, exit::ERROR);
    assert_eq!(
        err.trim(),
        "miyu: cannot pack: logo art/logo.png is not a PNG or WebP image"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

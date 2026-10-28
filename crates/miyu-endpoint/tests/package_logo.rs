//! 包自带的图标（施工 F-18 上，`[package] logo`）：真核心走一遍。合规矩的（PNG / WebP、不超过 256 KiB、每边不超过 512）
//! `package.list` 带版本（内容的 SHA-256 前 16 位），`package.logo` 读得到；不在、不是图、超了的当没有图标、
//! 包照装，`miyu check` 报警告；没装的包 `unknown_package`。

use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use miyu_session::testkit::Script;

use crate::support::*;

/// 带图标 `art/logo.png` 的包。
const KIT: &str = "[package]\nname = { en = \"Kit\" }\nlogo = \"art/logo.png\"\n";

/// 一张 `width`×`height` 的 PNG。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(width, height, Rgba([1, 2, 3, 255])))
        .write_to(&mut out, ImageFormat::Png)
        .expect("编得了");
    out.into_inner()
}

/// 一张 `side`×`side` 的噪点 PNG：边长在上限以内，字节超过 256 KiB（压不动）。
fn noise(side: u32) -> Vec<u8> {
    let mut state: u32 = 0x9e37_79b9;
    let image = RgbaImage::from_fn(side, side, |_, _| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        Rgba(state.to_le_bytes())
    });
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut out, ImageFormat::Png)
        .expect("编得了");
    out.into_inner()
}

/// 一张 JPEG（不是 PNG、WebP 的图）。
fn jpeg() -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3])))
        .write_to(&mut out, ImageFormat::Jpeg)
        .expect("编得了");
    out.into_inner()
}

/// 放一个带图标的包 `id`，图标的字节是 `logo`（没有的不放）。
fn put(home: &Home, id: &str, logo: Option<&[u8]>) {
    home.write(&format!("packages/{id}/package.toml"), KIT);
    if let Some(logo) = logo {
        let path = home.root.packages().join(id).join("art/logo.png");
        std::fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
        std::fs::write(path, logo).expect("写得进");
    }
}

/// `package.list` 里编号是 `id` 的那一项。
async fn listed(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|item| item["package"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

#[tokio::test]
async fn a_logo_is_listed_by_version_and_read_back() {
    let home = Home::new();
    let logo = png(64, 64);
    put(&home, "kit", Some(&logo));
    let core = home.core(&Script::new([]));
    let mut owner = Client::connect(std::sync::Arc::clone(&core));
    owner.hello().await;
    let version: String = Sha256::digest(&logo)
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(listed(&mut owner, "kit").await["logo"], json!(version));
    let read = owner
        .call("r", "package.logo", json!({"package": "kit"}))
        .await;
    assert_eq!(
        read["result"],
        json!({"package": "kit", "logo": version, "media_type": "image/png", "data": STANDARD.encode(&logo)}),
        "{read}"
    );
    let none = owner
        .call("n", "package.logo", json!({"package": "nothing"}))
        .await;
    assert_eq!(none["error"]["data"]["reason"], "unknown_package", "{none}");
}

#[tokio::test]
async fn a_logo_that_breaks_the_rules_is_left_out_and_reported() {
    let home = Home::new();
    put(&home, "wide", Some(&png(600, 10)));
    put(&home, "fake", Some(b"not a picture"));
    put(&home, "gone", None);
    let heavy = noise(400);
    assert!(heavy.len() > 256 * 1024, "噪点图得超字节：{}", heavy.len());
    put(&home, "heavy", Some(&heavy));
    put(&home, "jpeg", Some(&jpeg()));
    put(&home, "folder", None);
    std::fs::create_dir_all(home.root.packages().join("folder/art/logo.png")).expect("建得了");
    put(&home, "fine", Some(&png(16, 16)));
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    for id in ["wide", "fake", "gone", "heavy", "jpeg", "folder"] {
        let item = listed(&mut client, id).await;
        assert!(item.get("logo").is_none(), "不合规矩的不带：{item}");
        assert!(item.get("problem").is_none(), "包照装：{item}");
        let read = client
            .call("r", "package.logo", json!({"package": id}))
            .await;
        assert_eq!(
            read["result"],
            json!({"package": id, "logo": null, "media_type": null, "data": null}),
            "{read}"
        );
    }
    let checked = client.call("c", "check", json!({})).await;
    let mut found: Vec<(String, String, String)> = checked["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{checked}"))
        .iter()
        .filter(|problem| {
            problem["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("logo_"))
        })
        .map(|problem| {
            let file = problem["file"].as_str().unwrap_or_default();
            let package = file.split('/').rev().nth(1).unwrap_or_default().to_string();
            (
                package,
                problem["code"].as_str().unwrap_or_default().to_string(),
                problem["level"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            ("fake".into(), "logo_not_image".into(), "warning".into()),
            ("folder".into(), "logo_missing".into(), "warning".into()),
            ("gone".into(), "logo_missing".into(), "warning".into()),
            ("heavy".into(), "logo_too_big".into(), "warning".into()),
            ("jpeg".into(), "logo_not_image".into(), "warning".into()),
            ("wide".into(), "logo_too_big".into(), "warning".into()),
        ]
    );
}

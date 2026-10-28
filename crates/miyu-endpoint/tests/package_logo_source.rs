//! 软件源里的包自带的图标（施工 F-18 下）：真核心走一遍，假服务器当软件源。`package.search` 每一行带 `icon`、图标的版本；
//! `package.logo {"from"}` 照源的地址下 `logos/<哈希>.png`，存下以后不再下；位置写歪的、哈希对不上的不收，不留半截文件；
//! 不认识的源 `unknown_source`，索引里没有的 `unknown_package`。

use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};

use miyu_http::testkit::Server;

use crate::support::source::{bytes, core, entry, keys, serving, sha256, signed_entries};
use crate::support::*;

/// 一张 16×16 的 PNG。
fn png() -> Vec<u8> {
    png_of(16, 16)
}

/// 一张 `width`×`height` 的 PNG。
fn png_of(width: u32, height: u32) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(width, height, Rgba([9, 8, 7, 255])))
        .write_to(&mut out, ImageFormat::Png)
        .expect("编得了");
    out.into_inner()
}

/// 索引里的一条，带 Lucide 名和图标（位置照哈希算，`sha256` 是索引写的、`size` 照 `bytes`）。
fn with_logo(id: &str, sha256: &str, size: usize) -> Value {
    let mut one = entry(id, json!({"en": id}), json!({}));
    one["icon"] = json!("cat");
    one["logo"] = json!({"path": format!("logos/{sha256}.png"), "sha256": sha256, "size": size});
    one
}

#[tokio::test]
async fn a_logo_in_a_source_is_listed_by_version_and_fetched_once() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let logo = png();
    let good = sha256(&logo);
    let wrong = sha256(b"something else");
    let wide_bytes = png_of(600, 10);
    let mut crooked = with_logo("crooked", &good, logo.len());
    crooked["logo"]["path"] = json!(format!("../logos/{good}.png"));
    let index = signed_entries(
        "2026-10-28T03:00:00Z",
        &[
            with_logo("kit", &good, logo.len()),
            entry("plain", json!({"en": "plain"}), json!({})),
            crooked,
            with_logo("liar", &wrong, logo.len()),
            with_logo("huge", &good, 300 * 1024),
            with_logo("wide", &sha256(&wide_bytes), wide_bytes.len()),
        ],
        &secret,
    );
    let mut replies = serving(&index);
    replies.push(bytes(&logo));
    replies.push(bytes(&wide_bytes));
    replies.push(bytes(&logo));
    let server = Server::start(replies).await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;

    let found = client
        .call("q", "package.search", json!({"query": ""}))
        .await;
    let rows = found["result"]["packages"].as_array().expect("有");
    let row = |id: &str| {
        rows.iter()
            .find(|row| row["package"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("没有 {id}：{found}"))
    };
    assert_eq!(row("kit")["icon"], "cat");
    assert_eq!(row("kit")["logo"], json!(&good[..16]), "只给版本，不带图");
    assert!(row("plain").get("logo").is_none() && row("plain").get("icon").is_none());
    assert!(row("crooked").get("logo").is_none(), "位置写歪的不算有图标");

    let read = |id: &str| json!({"package": id, "from": "official"});
    let first = client.call("l", "package.logo", read("kit")).await;
    assert_eq!(
        first["result"],
        json!({"package": "kit", "logo": &good[..16], "media_type": "image/png", "data": STANDARD.encode(&logo)}),
        "{first}"
    );
    let asked = server.received().len();
    assert_eq!(
        server.received()[asked - 1].path,
        format!("/v1/logos/{good}.png")
    );
    let again = client.call("l", "package.logo", read("kit")).await;
    assert_eq!(again["result"], first["result"]);
    let crooked = client.call("c", "package.logo", read("crooked")).await;
    assert_eq!(crooked["result"]["logo"], Value::Null, "{crooked}");
    let huge = client.call("h", "package.logo", read("huge")).await;
    assert_eq!(
        huge["result"]["logo"],
        Value::Null,
        "索引写的超了上限：{huge}"
    );
    assert_eq!(
        server.received().len(),
        asked,
        "存下的不再下，位置写歪的、超了上限的不去下"
    );

    let wide = client.call("d", "package.logo", read("wide")).await;
    assert_eq!(wide["result"]["logo"], Value::Null, "下来的太宽：{wide}");

    let liar = client.call("x", "package.logo", read("liar")).await;
    assert_eq!(liar["error"]["data"]["reason"], "download_failed", "{liar}");
    let mut cached: Vec<String> = std::fs::read_dir(home.root.state().join("packages/.sync/logos"))
        .expect("有")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    cached.sort();
    let mut expected = [
        format!("{good}.png"),
        format!("{}.png", sha256(&wide_bytes)),
    ];
    expected.sort();
    assert_eq!(cached, expected, "哈希对不上的不留半截");

    let none = client.call("n", "package.logo", read("nothing")).await;
    assert_eq!(none["error"]["data"]["reason"], "unknown_package", "{none}");
    let nowhere = client
        .call(
            "w",
            "package.logo",
            json!({"package": "kit", "from": "nowhere"}),
        )
        .await;
    assert_eq!(
        nowhere["error"]["data"]["reason"], "unknown_source",
        "{nowhere}"
    );
}

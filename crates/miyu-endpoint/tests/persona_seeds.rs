//! 人格头像的候选主色（施工 P-6 补，`docs/blueprint/personas.md`「主题色、背景图」第 4 条）：真核心走一遍。`persona.list`、
//! `persona.get`、`persona.set` 的回应带 `seeds`，照 `miyu-seeds` 从头像挑，和两个头经门面算的一样；没有头像、解不开的是
//! `null`；换了头像跟着换。

use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::Script;

/// 一张真的 PNG：左边 `left` 列是 `a`，右边是 `b`。
fn picture(left: u32, a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
    let image = RgbaImage::from_fn(64, 64, |x, _| Rgba(if x < left { a } else { b }));
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("写得成 PNG");
    bytes.into_inner()
}

/// 照 `miyu-seeds` 算的候选，写成协议里的样子。
fn expected(bytes: &[u8]) -> Value {
    let seeds: Vec<String> = miyu_seeds::from_image(bytes)
        .expect("解得开")
        .into_iter()
        .map(miyu_seeds::Seed::hex)
        .collect();
    json!(seeds)
}

#[tokio::test]
async fn the_avatar_gives_its_seeds_and_a_new_avatar_new_ones() {
    let home = Home::new();
    let red = picture(48, [200, 30, 40, 255], [30, 60, 210, 255]);
    home.write(
        "home/alice/personas/base/persona.toml",
        "[persona]\nname = \"底\"\n",
    );
    std::fs::write(
        home.root.path().join("home/alice/personas/base/avatar.png"),
        &red,
    )
    .expect("写得进");
    home.write(
        "home/alice/personas/bare/persona.toml",
        "[persona]\nname = \"空\"\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;

    let got = client
        .call("g", "persona.get", json!({"persona": "base"}))
        .await;
    let seeds = &got["result"]["seeds"];
    assert_eq!(*seeds, expected(&red), "{got}");
    let count = seeds.as_array().map(Vec::len).unwrap_or_default();
    assert!((1..=4).contains(&count), "最多 4 个：{seeds}");
    assert!(
        seeds[0]
            .as_str()
            .is_some_and(|hex| hex.len() == 7 && hex.starts_with('#'))
    );

    let listed = client.call("l", "persona.list", json!({})).await;
    let row = |id: &str| {
        listed["result"]["personas"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["persona"] == id).cloned())
            .unwrap_or_else(|| panic!("{listed}"))
    };
    assert_eq!(row("base")["seeds"], expected(&red), "列出来也带");
    assert_eq!(row("bare")["seeds"], Value::Null, "没有头像的是 null");

    // 换一张蓝的多的：回应和再读到的都跟着换。
    let blue = picture(16, [200, 30, 40, 255], [30, 60, 210, 255]);
    let put = client
        .call(
            "p",
            "blob.put",
            json!({"data": STANDARD.encode(&blue), "name": "blue.png"}),
        )
        .await;
    let blob = put["result"]["blob"]
        .as_str()
        .expect("传上去了")
        .to_string();
    let changed = client
        .call(
            "s",
            "persona.set",
            json!({"persona": "base", "avatar": {"blob": blob}}),
        )
        .await;
    assert_eq!(changed["result"]["seeds"], expected(&blue), "{changed}");
    assert_ne!(
        changed["result"]["seeds"],
        expected(&red),
        "换了头像，候选跟着换"
    );
    let again = client
        .call("g2", "persona.get", json!({"persona": "base"}))
        .await;
    assert_eq!(again["result"]["seeds"], expected(&blue));
}

#[tokio::test]
async fn an_avatar_that_does_not_decode_has_no_seeds() {
    let home = Home::new();
    home.write(
        "home/alice/personas/base/persona.toml",
        "[persona]\nname = \"底\"\n",
    );
    // 头只有 PNG 的开头：版本照样有，解不开，候选是 null。
    let head = crate::attach::png(32, 32);
    std::fs::write(
        home.root.path().join("home/alice/personas/base/avatar.png"),
        &head,
    )
    .expect("写得进");
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let got = client
        .call("g", "persona.get", json!({"persona": "base"}))
        .await;
    assert!(got["result"]["avatar"].is_string(), "{got}");
    assert_eq!(got["result"]["seeds"], Value::Null, "{got}");
}

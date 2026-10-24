//! 给别的包带的内容（施工 X-22，`content.list`、`content.read`、`content.changed`）：写了 `[content.memes]` 的包，`memes` 的扩展
//! 列得出、读得到那个目录里的，读不到目录外的、别的包的；头调不了。装、卸给它带内容的包推 `content.changed`，跟它不相干的装卸
//! 不推。

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::*;

/// 给 `memes` 带内容的包：版本 1.0.0，内容在 `stickers/` 下。
const PACK: &str = "[package]\nversion = \"1.0.0\"\nname = { en = \"Pack\" }\n\n[content.memes]\ndir = \"stickers\"\n";

/// 给别的包带内容的包。
const OTHER: &str = "[package]\nname = { en = \"Other\" }\n\n[content.someone]\ndir = \"x\"\n";

/// 记下的每一行里能读成 JSON 的。
fn said(path: &std::path::Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// 编号是 `id` 的那一条回应。
fn reply(path: &std::path::Path, id: &str) -> Value {
    said(path)
        .into_iter()
        .find(|line| line["id"] == id)
        .unwrap_or_else(|| panic!("没有 {id} 的回应：{}", read(path)))
}

/// 推来的 `content.changed` 有几条。
fn pushes(path: &std::path::Path) -> usize {
    said(path)
        .iter()
        .filter(|line| line["method"] == "content.changed")
        .count()
}

/// 等到推来的 `content.changed` 有 `n` 条。
async fn until_pushes(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while pushes(path) < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 条：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn an_extension_reads_the_content_other_packages_carry_for_it() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "memes");
    let asks = [
        "call:content.list".to_string(),
        r#"ask:content.read:{"package":"pack","path":"index.json"}"#.to_string(),
        r#"ask:content.read:{"package":"pack","path":"../package.toml"}"#.to_string(),
        r#"ask:content.read:{"package":"pack","path":""}"#.to_string(),
        r#"ask:content.read:{"package":"other","path":"a"}"#.to_string(),
        r#"ask:content.read:{"package":"pack","path":"missing.json"}"#.to_string(),
        r#"ask:content.read:{"package":"pack","path":"up/secret.txt"}"#.to_string(),
    ];
    let mut all = vec![step, "hello".to_string()];
    all.extend(asks);
    all.push("serve".to_string());
    let all: Vec<&str> = all.iter().map(String::as_str).collect();
    install(&home, "memes", &program.name(), "always", &steps(&all));
    home.write("system/packages/pack/package.toml", PACK);
    home.write("system/packages/pack/stickers/index.json", "{\"n\": 1}");
    home.write("system/packages/pack/secret.txt", "nope");
    home.write("system/packages/other/package.toml", OTHER);
    home.write("system/packages/other/x/a", "a");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        home.root.path().join("system/packages/pack"),
        home.root.path().join("system/packages/pack/stickers/up"),
    )
    .expect("建得了链接");
    let core = core(&home, quick());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while said(&path)
        .iter()
        .filter(|line| line.get("id").is_some())
        .count()
        < 8
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到回应：{}",
            read(&path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(
        reply(&path, "s2")["result"],
        json!({"packages": [{"package": "pack", "version": "1.0.0"}]}),
        "只列给它带了内容的，不带给别人的"
    );
    let index = &reply(&path, "s3")["result"];
    assert_eq!(index["data"], STANDARD.encode("{\"n\": 1}"));
    assert_eq!(
        (index["size"].as_u64(), index["eof"].as_bool()),
        (Some(8), Some(true))
    );
    for (id, reason) in [
        ("s4", "bad_params"),
        ("s5", "bad_params"),
        ("s6", "unknown_package"),
        ("s7", "not_found"),
    ] {
        let got = reply(&path, id);
        assert_eq!(got["error"]["data"]["reason"], reason, "{id}：{got}");
    }
    #[cfg(unix)]
    assert_eq!(
        reply(&path, "s8")["error"]["data"]["reason"],
        "not_found",
        "经指出目录的链接读不到"
    );

    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    let head = client.call("h1", "content.list", json!({})).await;
    assert_eq!(head["error"]["data"]["reason"], "not_a_provider", "{head}");
    assert_eq!(pushes(&path), 0, "没装卸不推");

    let removed = client
        .call("r1", "package.remove", json!({"package": "pack"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    until_pushes(&path, 1).await;

    let source = home.work.join("pack2");
    std::fs::create_dir_all(source.join("stickers")).expect("建得了");
    std::fs::write(source.join("package.toml"), PACK).expect("写得进");
    let installed = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(installed["result"]["package"], "pack2", "{installed}");
    until_pushes(&path, 2).await;
    let removed = client
        .call("r2", "package.remove", json!({"package": "pack2"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    until_pushes(&path, 3).await;

    // 不相干的装卸：前后都没有给它带内容的包，不推。再装回一个带内容的，推的是第四条，中间那一下没推。
    let unrelated = home.work.join("unrelated");
    std::fs::create_dir_all(&unrelated).expect("建得了");
    std::fs::write(unrelated.join("package.toml"), OTHER).expect("写得进");
    let quiet = client
        .call("i2", "package.install", json!({"path": unrelated}))
        .await;
    assert_eq!(quiet["result"]["package"], "unrelated", "{quiet}");
    let loud = client
        .call("i3", "package.install", json!({"path": source}))
        .await;
    assert_eq!(loud["result"]["package"], "pack2", "{loud}");
    until_pushes(&path, 4).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(pushes(&path), 4, "不相干的那一下没推");
}

//! `package.info` 装着的照本地、没装的照软件源（施工 IN-2，设计 `49-装包照pacman.md` 第一节、第三节第 2 条）：真核心走一遍，假
//! 服务器当软件源。`source: true` 只查软件源、`false` 只查装着的；两处都没有的 `unknown_package`；装着的多清单写的几格和
//! 「被依赖」。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_http::testkit::Server;

use crate::support::source::{core, entry, keys, serving, signed_entries};
use crate::support::*;

/// 软件源里的 `memes`（1.0，带全了 IN-1 那四格）和只有编号的 `plain`。
fn index(secret: &str) -> (Vec<u8>, Vec<u8>) {
    let mut memes = entry(
        "memes",
        json!({"en": "Memes", "zh": "表情包"}),
        json!({"en": "Stickers"}),
    );
    memes["depends"] = json!(["im"]);
    memes["recommends"] = json!(["im-sponsor", "extra"]);
    memes["provides"] = json!({"command": ["memes"], "skills": ["sticker"]});
    memes["capabilities"] = json!(["network"]);
    memes["author"] = json!("miyu-agent");
    memes["upstream"] = json!("https://github.com/miyu-agent/memes");
    memes["installed"] = json!(6_800_000);
    memes["files"] =
        json!([{"url": "https://example.invalid/memes.miyu", "sha256": "ab", "size": 2_200_000}]);
    signed_entries(
        "2026-10-30T03:00:00Z",
        &[memes, entry("plain", json!({"en": "Plain"}), json!({}))],
        secret,
    )
}

/// 工作目录里的一个包文件夹：清单 `manifest`，另放 `files`。
fn folder(home: &Home, id: &str, manifest: &str, files: &[(&str, &str)]) -> PathBuf {
    let folder = home.work.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), manifest).expect("写得进");
    for (path, text) in files {
        let path = folder.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级")).expect("建得了");
        std::fs::write(path, text).expect("写得进");
    }
    folder
}

async fn info(client: &mut Client, params: Value) -> Value {
    client.call("i", "package.info", params).await
}

#[tokio::test]
async fn a_package_not_installed_is_answered_from_the_source() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let server = Server::start(serving(&index(&secret))).await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;
    let got = info(&mut client, json!({"package": "memes"})).await;
    let got = &got["result"];
    assert_eq!(got["from"], "source", "{got}");
    assert_eq!(got["source"], "official");
    assert_eq!(got["package"], "memes");
    assert_eq!(got["version"], "1.0");
    assert_eq!(got["name"], "表情包", "照连接的语言挑");
    assert_eq!(got["summary"], "Stickers", "没有这种语言的照英文");
    assert_eq!(got["depends"], json!(["im"]));
    assert_eq!(got["recommends"], json!(["im-sponsor", "extra"]));
    assert_eq!(
        got["provides"],
        json!({"command": ["memes"], "skills": ["sticker"]}),
        "索引原样"
    );
    assert_eq!(got["capabilities"][0]["id"], "network");
    assert_eq!(got["capabilities"][0]["name"], "联网", "能力照连接的语言说");
    assert_eq!(got["origin"], "github.com/miyu-agent/memes");
    assert_eq!(got["official"], true);
    assert_eq!(got["author"], "miyu-agent");
    assert_eq!(got["upstream"], "https://github.com/miyu-agent/memes");
    assert_eq!(got["download"], 2_200_000, "本机那份文件多大");
    assert_eq!(got["installed_size"], 6_800_000);
    assert!(got.get("stale").is_none(), "拿到了新的：{got}");
    assert!(got.get("layer").is_none(), "没装的没有装着的那几格：{got}");

    // 以前生成的索引没有的不写。
    let plain = info(&mut client, json!({"package": "plain", "source": true})).await;
    let plain = &plain["result"];
    assert_eq!(plain["from"], "source", "{plain}");
    for key in ["author", "upstream", "installed_size", "summary"] {
        assert!(plain.get(key).is_none(), "{key}：{plain}");
    }
    assert_eq!(plain["recommends"], json!([]));
    assert_eq!(server.received().len(), 2, "查几次只拿一次索引");
}

#[tokio::test]
async fn source_true_does_not_look_at_the_installed_one_and_false_does_not_look_at_the_source() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let server = Server::start(serving(&index(&secret))).await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;
    let memes = folder(
        &home,
        "memes",
        "[package]\nversion = \"0.9\"\nname = { en = \"Memes\" }\n",
        &[],
    );
    let installed = client
        .call("p", "package.install", json!({"path": memes}))
        .await;
    assert_eq!(installed["result"]["package"], "memes", "{installed}");

    let local = info(&mut client, json!({"package": "memes"})).await;
    assert_eq!(
        local["result"]["version"], "0.9",
        "不写的先查装着的：{local}"
    );
    assert_eq!(local["result"]["layer"], "home");
    let remote = info(&mut client, json!({"package": "memes", "source": true})).await;
    assert_eq!(remote["result"]["version"], "1.0", "只查软件源：{remote}");
    assert_eq!(remote["result"]["from"], "source");
    let only_local = info(&mut client, json!({"package": "plain", "source": false})).await;
    assert_eq!(
        reason(&only_local),
        Some("unknown_package"),
        "只查装着的不去软件源：{only_local}"
    );
}

#[tokio::test]
async fn an_installed_package_carries_its_manifest_and_who_needs_it() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let server = Server::start(serving(&index(&secret))).await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;
    let base = folder(
        &home,
        "base",
        "[package]\nversion = \"1.0\"\nname = { en = \"Base\", zh = \"底座\" }\nsummary = { en = \"The base\" }\nauthor = \"me\"\nupstream = \"https://example.invalid/base\"\n\n[recommends]\npackages = [\"nice\"]\n",
        &[(
            "skills/draw/SKILL.md",
            "---\nname: draw\ndescription: Draw things.\n---\nbody\n",
        )],
    );
    let addon = folder(
        &home,
        "addon",
        "[package]\nversion = \"1.0\"\nname = { en = \"Addon\" }\n\n[depends]\npackages = [\"base\"]\n",
        &[],
    );
    for path in [&base, &addon] {
        let reply = client
            .call("p", "package.install", json!({"path": path}))
            .await;
        assert!(reply.get("result").is_some(), "{reply}");
    }
    let got = info(&mut client, json!({"package": "base"})).await;
    let got = &got["result"];
    // 原来的几格照旧。
    assert_eq!(got["layer"], "home", "{got}");
    assert_eq!(got["from"], "local");
    assert_eq!(got["source"], base.display().to_string());
    assert!(got["installed"].is_string());
    assert!(got["files"].as_u64().is_some_and(|n| n >= 2));
    // 多的几格。
    assert_eq!(got["name"], "底座");
    assert_eq!(got["summary"], "The base");
    assert_eq!(got["author"], "me");
    assert_eq!(got["upstream"], "https://example.invalid/base");
    assert_eq!(got["recommends"], json!(["nice"]));
    assert!(got.get("depends").is_none(), "没写依赖的不写：{got}");
    assert_eq!(got["required_by"], json!(["addon"]), "装着的包里依赖它的");
    assert_eq!(
        got["provides"]["skills"],
        json!(["draw"]),
        "同装之前的「包含」"
    );
    assert!(got.get("origin").is_none(), "不是从软件源装的没有来源仓库");

    let addon = info(&mut client, json!({"package": "addon"})).await;
    assert_eq!(addon["result"]["depends"], json!(["base"]), "{addon}");
    assert!(addon["result"].get("required_by").is_none());
}

#[tokio::test]
async fn nowhere_is_unknown_and_an_unreachable_source_is_refused_like_search() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let server = Server::start(serving(&index(&secret))).await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted.clone()));
    client.hello().await;
    let got = info(&mut client, json!({"package": "ghost"})).await;
    assert_eq!(reason(&got), Some("unknown_package"), "{got}");
    let got = info(&mut client, json!({"package": "ghost", "source": true})).await;
    assert_eq!(reason(&got), Some("unknown_package"), "{got}");

    // 软件源拿不到（签名对不上）、本机也没有：只查软件源的照搜的错，不写的说没有。
    let other = Home::new();
    let (wrong, _) = keys();
    let server = Server::start(serving(&index(&wrong))).await;
    let mut client = Client::connect(core(&other, &server.base_url, &[], trusted));
    client.hello().await;
    let got = info(&mut client, json!({"package": "memes", "source": true})).await;
    assert_eq!(reason(&got), Some("source_unreachable"), "{got}");
    let got = info(&mut client, json!({"package": "memes"})).await;
    assert_eq!(reason(&got), Some("unknown_package"), "{got}");
}

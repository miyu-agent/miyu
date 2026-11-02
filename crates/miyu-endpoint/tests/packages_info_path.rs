//! `package.info` 带 `path`（施工 IN-4，照 pacman 的 `-Qip`）：看本机一个包文件夹、包文件 `.miyu`，什么都不装；交回的照
//! `package.info` 装着的那一份的样子（`from: "file"`），另带写的那个路径。装不上的照真装那样拒：写错的、撞了名的；`package`、
//! `path` 只能写一个，带 `path` 的不收 `source`。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_source::archive;

use crate::support::*;

/// 一份带名字、说明、作者、依赖的清单（`bundle`）。
const KIT: &str = "[package]\nversion = \"1.2\"\nname = { en = \"Kit\", zh = \"工具包\" }\nsummary = { en = \"Does things\" }\nauthor = \"Ann\"\n";

/// 一份 `SKILL.md`。
fn skill(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Do {name}.\n---\nbody\n")
}

/// 工作目录里放一个包文件夹 `id`：清单 `manifest`，外加 `files`（相对包文件夹的路径和内容）。
fn folder(home: &Home, id: &str, manifest: &str, files: &[(&str, String)]) -> PathBuf {
    let folder = home.work.join("sources").join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), manifest).expect("写得进");
    for (path, text) in files {
        let file = folder.join(path);
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得了");
        std::fs::write(file, text).expect("写得进");
    }
    folder
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

async fn info(client: &mut Client, params: Value) -> Value {
    client.call("i", "package.info", params).await
}

fn placed(home: &Home, id: &str) -> bool {
    home.root.packages().join(id).exists()
}

#[tokio::test]
async fn a_folder_is_described_like_an_installed_package_and_nothing_moves() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let kit = folder(
        &home,
        "kit",
        KIT,
        &[
            ("skills/notes/SKILL.md", skill("notes")),
            ("skills/bad/SKILL.md", "no front matter\n".to_string()),
        ],
    );
    let shown = info(&mut client, json!({"path": kit})).await;
    let result = &shown["result"];
    assert_eq!(
        (
            result["from"].clone(),
            result["package"].clone(),
            result["version"].clone(),
            result["path"].clone(),
        ),
        (json!("file"), json!("kit"), json!("1.2"), json!(kit)),
        "{shown}"
    );
    assert_eq!(
        (result["summary"].clone(), result["author"].clone()),
        (json!("Does things"), json!("Ann")),
        "{shown}"
    );
    assert_eq!(result["provides"], json!({"skills": ["notes"]}), "{shown}");
    assert_eq!(
        result["skipped"][0]["file"], "skills/bad/SKILL.md",
        "{shown}"
    );
    assert_eq!(result["files"], 3, "{shown}");
    assert!(
        result["size"].as_u64().is_some_and(|size| size > 0),
        "{shown}"
    );
    assert!(result.get("layer").is_none(), "没装的没有哪一层：{shown}");
    assert!(!placed(&home, "kit"), "什么都不装");

    // 写到里面的 `package.toml` 的取它所在的文件夹，同装的那一套。
    let manifest = info(&mut client, json!({"path": kit.join("package.toml")})).await;
    assert_eq!(manifest["result"]["package"], "kit", "{manifest}");
}

#[tokio::test]
async fn a_package_file_is_unpacked_described_and_cleaned_up() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let kit = folder(
        &home,
        "kit",
        KIT,
        &[("skills/notes/SKILL.md", skill("notes"))],
    );
    let file = home.work.join("kit-1.2.miyu");
    archive::pack(&kit, "kit", &file).expect("打得成");
    let shown = info(&mut client, json!({"path": file})).await;
    let result = &shown["result"];
    assert_eq!(
        (
            result["from"].clone(),
            result["package"].clone(),
            result["path"].clone()
        ),
        (json!("file"), json!("kit"), json!(file)),
        "写的是包文件的路径，不是解开的暂存处：{shown}"
    );
    assert_eq!(result["provides"]["skills"], json!(["notes"]), "{shown}");
    assert!(!placed(&home, "kit"), "什么都不装");
    let unpacked = std::fs::read_dir(home.root.path().join("tmp/unpack"))
        .map(|dir| dir.count())
        .unwrap_or(0);
    assert_eq!(unpacked, 0, "暂存处用完就删");
}

#[tokio::test]
async fn what_would_not_install_is_refused_the_same_way() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let broken = folder(&home, "broken", "[package]\n\n[ui]\n\n[process]\n", &[]);
    let reply = info(&mut client, json!({"path": broken})).await;
    assert_eq!(reason(&reply), Some("package_invalid"), "{reply}");
    assert!(reply["error"]["data"]["line"].is_u64(), "带行号：{reply}");

    let first = folder(&home, "zkit", KIT, &[("skills/pdf/SKILL.md", skill("pdf"))]);
    let placed_first = client
        .call("p", "package.install", json!({"path": first}))
        .await;
    assert!(placed_first["result"].is_object(), "{placed_first}");
    let second = folder(&home, "akit", KIT, &[("skills/pdf/SKILL.md", skill("pdf"))]);
    let clash = info(&mut client, json!({"path": second})).await;
    assert_eq!(reason(&clash), Some("package_conflict"), "{clash}");
    assert_eq!(
        clash["error"]["data"]["conflicts"],
        json!([{"kind": "skill", "name": "pdf", "package": "zkit"}]),
        "{clash}"
    );
    assert!(!placed(&home, "akit"));
}

#[tokio::test]
async fn package_and_path_are_one_or_the_other_and_a_path_takes_no_source() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let kit = folder(&home, "kit", KIT, &[]);
    for params in [
        json!({"path": kit, "package": "kit"}),
        json!({"path": kit, "source": false}),
        json!({}),
        json!({"path": "sources/kit"}),
    ] {
        let reply = info(&mut client, params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
}

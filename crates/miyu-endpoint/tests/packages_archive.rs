//! 装包文件（施工 G-1，设计 `37-软件源.md` 第二节）：真核心走一遍。`package.install` 的 `path` 是 `.miyu` 文件的解开再装，本地库
//! 记的来源是那个文件；看一眼也认；解不开的 `package_file_bad`、什么都不装；暂存处用完就删。清单写了 `targets`、没有本机这一种
//! 的 `wrong_target`，文件夹、包文件一样。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_config::package::Target;
use miyu_session::testkit::Script;
use miyu_source::archive;

use crate::support::*;

/// 一份界面包的清单（不拉起程序），`more` 接在 `[package]` 里。
fn manifest(more: &str) -> String {
    format!(
        "[package]\nversion = \"1.0\"\nprotocol = [1, 1]\nname = {{ en = \"Tool\" }}\n{more}\n[command]\nname = \"xtool\"\nprogram = \"miyu-xtool\"\nabout = {{ en = \"X\" }}\n\n[ui]\n"
    )
}

/// 工作目录里的包文件夹 `xtool`（清单加一个文件），交回文件夹。
fn folder(home: &Home, more: &str) -> PathBuf {
    let folder = home.work.join("src").join("xtool");
    if folder.exists() {
        std::fs::remove_dir_all(&folder).expect("删得掉");
    }
    std::fs::create_dir_all(folder.join("bin")).expect("建得了");
    std::fs::write(folder.join("package.toml"), manifest(more)).expect("写得进");
    std::fs::write(folder.join("bin/data.txt"), "hi").expect("写得进");
    folder
}

/// 把 `folder` 打成工作目录里的 `name`。
fn packed(home: &Home, folder: &Path, name: &str) -> PathBuf {
    let out = home.work.join(name);
    archive::pack(folder, "xtool", &out).expect("打得成");
    out
}

/// 暂存处里还剩几样。
fn left_unpacked(home: &Home) -> usize {
    std::fs::read_dir(home.root.path().join("state/packages/.unpack"))
        .map(|dir| dir.count())
        .unwrap_or(0)
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

async fn install(client: &mut Client, params: Value) -> Value {
    client.call("i", "package.install", params).await
}

/// 本机以外的一种系统。
fn elsewhere() -> Target {
    Target::ALL
        .into_iter()
        .find(|target| Some(*target) != Target::here())
        .expect("有别的")
}

#[tokio::test]
async fn a_package_file_installs_and_is_recorded_as_its_source() {
    let home = Home::new();
    let file = packed(&home, &folder(&home, ""), "xtool-1.0.miyu");
    let mut client = connected(&home).await;

    let preview = install(&mut client, json!({"path": file, "preview": true})).await;
    assert_eq!(preview["result"]["package"], "xtool", "{preview}");
    assert_eq!(preview["result"]["files"], 2, "{preview}");
    assert!(
        !home.root.path().join("home/alice/packages/xtool").exists(),
        "看一眼什么都不装"
    );
    assert_eq!(left_unpacked(&home), 0, "看一眼的暂存处也删了");

    let reply = install(&mut client, json!({"path": file})).await;
    assert_eq!(reply["result"]["package"], "xtool", "{reply}");
    assert_eq!(
        std::fs::read_to_string(
            home.root
                .path()
                .join("home/alice/packages/xtool/bin/data.txt")
        )
        .expect("装进去了"),
        "hi"
    );
    let info = client
        .call("f", "package.info", json!({"package": "xtool"}))
        .await;
    assert_eq!(
        info["result"]["source"],
        file.display().to_string(),
        "{info}"
    );
    assert_eq!(info["result"]["files"], 2, "{info}");
    assert_eq!(left_unpacked(&home), 0, "暂存处用完就删");
}

#[tokio::test]
async fn a_bad_package_file_installs_nothing() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let text = home.work.join("broken.miyu");
    std::fs::write(&text, "not a package").expect("写得进");
    let reply = install(&mut client, json!({"path": text})).await;
    assert_eq!(reason(&reply), Some("package_file_bad"), "{reply}");
    assert_eq!(left_unpacked(&home), 0);
    assert!(!home.root.path().join("home/alice/packages/xtool").exists());

    let missing = install(&mut client, json!({"path": home.work.join("none.miyu")})).await;
    assert_eq!(reason(&missing), Some("path_unreadable"), "{missing}");
    let relative = install(&mut client, json!({"path": "xtool-1.0.miyu"})).await;
    assert_eq!(reason(&relative), Some("bad_params"), "{relative}");
}

#[tokio::test]
async fn a_package_for_another_system_is_refused() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let other = folder(
        &home,
        &format!("targets = [\"{}\"]\n", elsewhere().as_str()),
    );
    let reply = install(&mut client, json!({"path": other})).await;
    assert_eq!(reason(&reply), Some("wrong_target"), "{reply}");
    let file = packed(&home, &other, "xtool-other.miyu");
    let reply = install(&mut client, json!({"path": file})).await;
    assert_eq!(reason(&reply), Some("wrong_target"), "{reply}");
    assert!(!home.root.path().join("home/alice/packages/xtool").exists());
    assert_eq!(left_unpacked(&home), 0);

    if let Some(here) = Target::here() {
        let mine = folder(
            &home,
            &format!(
                "targets = [\"{}\", \"{}\"]\n",
                elsewhere().as_str(),
                here.as_str()
            ),
        );
        let reply = install(&mut client, json!({"path": mine})).await;
        assert_eq!(
            reply["result"]["package"], "xtool",
            "写了本机这一种的照装：{reply}"
        );
    }
}

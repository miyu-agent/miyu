//! `package.list` 每一项的 `provides`（施工 PV-2）：带了哪几样，形状同软件源索引那一格（种类到名字）。装上的带技能、脚本、MCP
//! 服务的各自对；出厂的 base-* 也带；`package.changed` 推的那一项、装的回应同样有。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 脚本的开头。
const SCRIPT: &str = "#!/bin/sh\n# Description: Say hello.\necho hello\n";

/// 工作目录里的一个包文件夹：清单 `[package]` 后面接 `more`，另放 `files`。
fn folder(home: &Home, id: &str, more: &str, files: &[(&str, &str)]) -> PathBuf {
    let folder = home.work.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        format!("[package]\nname = {{ en = \"{id}\" }}\n{more}"),
    )
    .expect("写得进");
    for (path, text) in files {
        let path = folder.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级")).expect("建得了");
        std::fs::write(path, text).expect("写得进");
    }
    folder
}

/// `package.list` 里编号是 `id` 的那一项。
fn item<'a>(listed: &'a Value, id: &str) -> &'a Value {
    listed["result"]["packages"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["package"] == id))
        .unwrap_or_else(|| panic!("列表里没有 {id}：{listed}"))
}

#[tokio::test]
async fn each_listed_package_says_what_it_provides() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut watching = Client::connect(Arc::clone(&core));
    watching.hello().await;
    let mut acting = Client::connect(Arc::clone(&core));
    acting.hello().await;
    let subscribed = watching
        .call("s", "subscribe", json!({"stream": "packages"}))
        .await;
    assert!(subscribed.get("result").is_some(), "{subscribed}");

    let cases = [
        (
            folder(
                &home,
                "sketch",
                "",
                &[(
                    "skills/draw/SKILL.md",
                    "---\nname: draw\ndescription: Draw things.\n---\nbody\n",
                )],
            ),
            "sketch",
            json!({"skills": ["draw"]}),
        ),
        (
            folder(&home, "greet", "", &[("scripts/hello.sh", SCRIPT)]),
            "greet",
            json!({"scripts": ["hello.sh"]}),
        ),
        (
            folder(
                &home,
                "probe",
                "\n[[mcp]]\nname = \"probe\"\ncommand = [\"miyu-no-such-program-anywhere\"]\n",
                &[],
            ),
            "probe",
            json!({"mcp": ["probe"]}),
        ),
    ];
    for (path, id, provides) in &cases {
        let installed = acting
            .call("i", "package.install", json!({"path": path}))
            .await;
        assert_eq!(
            installed["result"]["provides"], *provides,
            "装的回应同列表的一项：{installed}"
        );
        let pushed = package_changed(&mut watching, id).await;
        assert_eq!(pushed["provides"], *provides, "推的那一项：{pushed}");
    }

    let listed = acting.call("l", "package.list", json!({})).await;
    for (_, id, provides) in &cases {
        assert_eq!(item(&listed, id)["provides"], *provides, "{id}");
    }
    // 出厂的照同一份规则：文件夹里的技能、人格，清单写明的功能。
    assert_eq!(
        item(&listed, "base-authoring")["provides"]["skills"],
        json!(["package-creator", "skill-creator"])
    );
    assert_eq!(
        item(&listed, "base-persona-engineer")["provides"]["personas"],
        json!(["software-engineer"])
    );
    assert_eq!(
        item(&listed, "base-shell")["provides"]["features"],
        json!(["shell"])
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_package_that_provides_nothing_has_an_empty_table() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let plain = folder(&home, "plain", "", &[]);
    let installed = client
        .call("i", "package.install", json!({"path": plain}))
        .await;
    assert_eq!(
        installed["result"]["provides"],
        json!({}),
        "同索引：什么都没带的是空表：{installed}"
    );
}

/// 知识库（施工 PV-3）：包里 `kb/` 下编号写法对的目录是一个库（`kb.toml` 可以没有，同核心认包里的库），`package.list` 的
/// `provides`、`package.info` 的 `contains` 都列 `knowledge`；写法不对的目录、`kb/` 下的文件不列。许可证照清单原样交出去，
/// 装着的、带 `path` 的都有，没写的没有。
#[tokio::test]
async fn knowledge_bases_are_provided_and_the_license_is_handed_over() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let lore = folder(
        &home,
        "lore",
        "license = \"CC-BY-SA-4.0\"\n",
        &[
            ("kb/lighthouse/kb.toml", "name = \"Lighthouse\"\n"),
            ("kb/lighthouse/keeper.md", "# Keeper\n"),
            ("kb/atlas/map.md", "# Map\n"),
            ("kb/Bad Name/kb.toml", ""),
            ("kb/readme.md", "not a library\n"),
        ],
    );
    let looked = client
        .call("f", "package.info", json!({"path": lore}))
        .await;
    assert_eq!(
        looked["result"]["contains"]["knowledge"],
        json!(["atlas", "lighthouse"]),
        "装之前看一眼：{looked}"
    );
    assert_eq!(looked["result"]["license"], "CC-BY-SA-4.0", "{looked}");
    let installed = client
        .call("i", "package.install", json!({"path": lore}))
        .await;
    assert_eq!(
        installed["result"]["provides"],
        json!({"knowledge": ["atlas", "lighthouse"]}),
        "{installed}"
    );
    let listed = client.call("l", "package.list", json!({})).await;
    assert_eq!(
        item(&listed, "lore")["provides"]["knowledge"],
        json!(["atlas", "lighthouse"])
    );
    let info = client
        .call("q", "package.info", json!({"package": "lore"}))
        .await;
    assert_eq!(
        info["result"]["contains"]["knowledge"],
        json!(["atlas", "lighthouse"]),
        "{info}"
    );
    assert_eq!(info["result"]["license"], "CC-BY-SA-4.0", "{info}");

    let plain = folder(&home, "plain", "", &[]);
    let looked = client
        .call("g", "package.info", json!({"path": plain}))
        .await;
    assert!(looked["result"].is_object(), "{looked}");
    assert!(looked["result"].get("license").is_none(), "{looked}");
    assert!(looked["result"].get("contains").is_none(), "{looked}");
}

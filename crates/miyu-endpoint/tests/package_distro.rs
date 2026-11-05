//! 系统加装那一层（施工 SD-1，设计 60 第三节）：真核心走一遍。系统加装的照列、来源是 `distro`；同编号装上的盖住它，被盖住
//! 的不算错、说清被谁盖住、两边的版本和新不新，`miyu check` 报一条警告；`package.check` 只查清单和 `bin/` 的程序；卸系统
//! 加装的只记一笔、文件不动。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::*;

/// 一份界面清单：编号、子命令名都是 `id`，程序 `miyu-<id>`，版本 `version`。
fn ui(id: &str, version: &str) -> String {
    format!(
        "[package]\nversion = \"{version}\"\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"miyu-{id}\"\nabout = {{ en = \"Open {id}\" }}\n\n[ui]\n"
    )
}

/// 系统加装那一处：数据根旁边的临时目录（测试的家目录里，不是核心认的位置）。
fn distro(home: &Home) -> PathBuf {
    home.root.path().join("tmp").join("distro-test")
}

/// 在系统加装那一处写一份。
fn put(at: &Path, file: &str, text: &str) {
    let path = at.join(file);
    std::fs::create_dir_all(path.parent().expect("测试的前提")).expect("测试的前提");
    std::fs::write(path, text).expect("测试的前提");
}

/// 照这一处起一个核心：起来时读的清单、重读的位置都带上它。
fn core(home: &Home) -> std::sync::Arc<miyu_endpoint::Core> {
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root, vec![distro(home)]);
    std::sync::Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_distro(vec![distro(home)])
            .with_packages(found),
    )
}

/// 这几个编号的项，照列出的先后。
fn only(reply: &Value, id: &str) -> Vec<Value> {
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|item| item["package"] == id)
        .cloned()
        .collect()
}

#[tokio::test]
async fn distro_packages_are_listed_shadowed_checked_and_removed_without_touching_files() {
    let home = Home::new();
    let at = distro(&home);
    // dview：只有系统加装的一份，程序在 bin/。
    put(&at, "dview/package.toml", &ui("dview", "1.0.0"));
    put(&at, "dview/bin/miyu-dview", "#!/bin/sh\n");
    put(
        &at,
        &format!("dview/bin/miyu-dview{}", std::env::consts::EXE_SUFFIX),
        "#!/bin/sh\n",
    );
    // dterm：系统加装的 0.3.0 被装上的 0.2.0 盖住。
    put(&at, "dterm/package.toml", &ui("dterm", "0.3.0"));
    home.write("opt/dterm/package.toml", &ui("dterm", "0.2.0"));
    let mut client = Client::connect(core(&home));
    client.hello().await;

    let listed = client.call("l1", "package.list", json!({})).await;
    let dview = only(&listed, "dview");
    assert_eq!(dview.len(), 1, "{listed}");
    assert_eq!(
        (dview[0]["layer"].as_str(), dview[0]["from"].as_str()),
        (Some("distro"), Some("distro"))
    );
    assert!(dview[0].get("shadowed").is_none());
    let dterm = only(&listed, "dterm");
    assert_eq!(dterm.len(), 2, "{dterm:?}");
    assert_eq!(dterm[0]["layer"], "home");
    assert_eq!(dterm[1]["layer"], "distro");
    assert_eq!(
        dterm[1]["shadowed"],
        json!({"by": "home", "version": "0.3.0", "over": "0.2.0", "newer": true})
    );
    assert!(dterm[1].get("code").is_none(), "被盖住的不算错");

    let checked = client.call("c1", "check", json!({})).await;
    let covered: Vec<&Value> = checked["result"]["problems"]
        .as_array()
        .expect("测试的前提")
        .iter()
        .filter(|problem| problem["code"] == "shadowed")
        .collect();
    assert_eq!(covered.len(), 1, "{checked}");
    assert_eq!(covered[0]["level"], "warning");
    let message = covered[0]["message"].as_str().expect("测试的前提");
    assert!(
        message.contains("0.3.0") && message.contains("0.2.0"),
        "{message}"
    );
    assert!(
        message.contains("miyu pkg remove dterm"),
        "这一份更新：卸掉装上的那份换回来：{message}"
    );

    let files = client.call("k1", "package.check", json!({})).await;
    let entry = only(&files, "dview");
    assert_eq!(
        entry,
        [
            json!({"package": "dview", "layer": "distro", "modified": [], "missing": [], "extra": []})
        ],
        "{files}"
    );
    assert!(
        only(&files, "dterm")
            .iter()
            .all(|entry| entry.get("layer").is_none()),
        "认的是装上的那份：照本地库查，被盖住的系统加装那份不查：{files}"
    );
    std::fs::remove_file(at.join(format!(
        "dview/bin/miyu-dview{}",
        std::env::consts::EXE_SUFFIX
    )))
    .expect("测试的前提");
    let files = client
        .call("k2", "package.check", json!({"package": "dview"}))
        .await;
    assert_eq!(
        only(&files, "dview")[0]["missing"],
        json!([format!("bin/miyu-dview{}", std::env::consts::EXE_SUFFIX)]),
        "{files}"
    );

    let removed = client
        .call("r1", "package.remove", json!({"package": "dview"}))
        .await;
    assert_eq!(
        removed["result"],
        json!({"package": "dview", "removed": true, "layer": "distro"}),
        "{removed}"
    );
    assert!(
        at.join("dview/package.toml").is_file(),
        "系统加装的文件不动"
    );
    let listed = client.call("l2", "package.list", json!({})).await;
    let dview = only(&listed, "dview");
    assert!(
        dview.iter().all(|item| item["removed"] == true),
        "卸掉的只当装回的候选列：{dview:?}"
    );
}

#[tokio::test]
async fn a_broken_distro_manifest_is_reported_by_package_check() {
    let home = Home::new();
    let at = distro(&home);
    put(&at, "dbad/package.toml", "[package]\n\n[ui]\n\n[process]\n");
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let files = client
        .call("k1", "package.check", json!({"package": "dbad"}))
        .await;
    assert_eq!(
        only(&files, "dbad")[0]["manifest"],
        "two_programs",
        "{files}"
    );
}

#[tokio::test]
async fn a_removed_distro_package_comes_back_by_its_id() {
    let home = Home::new();
    let at = distro(&home);
    put(&at, "dkit/package.toml", &ui("dkit", "1.2.0"));
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let gone = client
        .call("r1", "package.remove", json!({"package": "dkit"}))
        .await;
    assert_eq!(gone["result"]["removed"], true, "{gone}");
    let marker = home.root.removed_markers().join("dkit");
    assert!(marker.exists(), "记号写上了");

    // 只写编号：照装回走，不去软件源（施工 SD-1补）。看一眼的同出厂的，`restores: true`、版本照系统加装的那份。
    let preview = client
        .call(
            "p1",
            "package.install",
            json!({"package": "dkit", "preview": true}),
        )
        .await;
    assert_eq!(preview["result"]["restores"], true, "{preview}");
    assert_eq!(preview["result"]["version"], "1.2.0", "{preview}");
    let back = client
        .call("i1", "package.install", json!({"package": "dkit"}))
        .await;
    assert_eq!(back["result"]["package"], "dkit", "{back}");
    assert_eq!(back["result"]["layer"], "distro", "{back}");
    assert!(
        back["result"]
            .get("removed")
            .is_none_or(|removed| removed == false),
        "{back}"
    );
    assert!(!marker.exists(), "装回去掉记号");
    let listed = client.call("l1", "package.list", json!({})).await;
    let dkit = only(&listed, "dkit");
    assert_eq!(dkit.len(), 1, "{dkit:?}");
    assert!(
        dkit[0]
            .get("removed")
            .is_none_or(|removed| removed == false),
        "{dkit:?}"
    );

    // 没卸过的再装：同出厂的没卸过的，`unknown_package`，不去软件源。
    let again = client
        .call("i2", "package.install", json!({"package": "dkit"}))
        .await;
    assert_eq!(
        again["error"]["data"]["reason"], "unknown_package",
        "{again}"
    );
}

#[tokio::test]
async fn bringing_a_distro_package_back_checks_its_own_names() {
    let home = Home::new();
    let at = distro(&home);
    put(&at, "dkit/package.toml", &ui("dkit", "1.0.0"));
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let gone = client
        .call("r1", "package.remove", json!({"package": "dkit"}))
        .await;
    assert_eq!(gone["result"]["removed"], true, "{gone}");
    // 卸掉以后装了一个子命令也叫 dkit 的包：装回系统加装那份前照它的清单查，撞了拒。
    let other = home.root.path().join("tmp").join("other");
    put(
        &other,
        "package.toml",
        &ui("dkit", "1.0.0").replace("program = \"miyu-dkit\"", "program = \"miyu-other\""),
    );
    let put_in = client
        .call(
            "i1",
            "package.install",
            json!({"path": other.to_string_lossy()}),
        )
        .await;
    assert!(put_in.get("result").is_some(), "{put_in}");
    for (n, preview) in [("p1", true), ("i2", false)] {
        let back = client
            .call(
                n,
                "package.install",
                json!({"package": "dkit", "preview": preview}),
            )
            .await;
        assert_eq!(
            back["error"]["data"]["reason"], "package_conflict",
            "{back}"
        );
    }
    assert!(
        home.root.removed_markers().join("dkit").exists(),
        "拒了不去掉记号"
    );
}

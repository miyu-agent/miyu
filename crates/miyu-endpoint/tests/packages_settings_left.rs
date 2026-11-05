//! 卸包也留配置里的设置项（施工 PK-5补，2026-11-05 项目主人定照 pacman）：真核心、照 `miyu-core` 拼配置清单的端口走一遍。
//! 不带 `purge` 卸的，`etc/config.toml` 里它的设置项原样留着、不生效，`check` 不报不认识、一个包说一句 `settings_left`，
//! 重启也一样；重装同编号照原样生效、记号删掉，前缀变了的以新清单为准；`purge` 的照旧删。没装的编号带 `purge` 只删留下的。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::packages::settings_left;

use crate::support::ported::{started, started_with};
use crate::support::*;

/// 一份界面包的清单：前缀写了的照写的，配置项 `port` 写在系统配置里。
fn manifest(prefix: Option<&str>) -> String {
    let prefix = prefix.map_or_else(String::new, |prefix| format!("prefix = \"{prefix}\"\n"));
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Cfg\" }}\n{prefix}\n[ui]\n\n[settings.port]\ntype = \"int\"\ndefault = 8400\nlayers = [\"system\"]\nname = {{ en = \"Port\" }}\n"
    )
}

/// 数据根外面的包文件夹 `<名字>/xcfg`，交回它。
fn source(home: &Home, name: &str, prefix: Option<&str>) -> PathBuf {
    let folder = home.work.join(name).join("xcfg");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), manifest(prefix)).expect("写得进");
    folder
}

async fn call(client: &mut Client, method: &str, params: Value) -> Value {
    client.call("c", method, params).await
}

/// `check` 报的配置的问题：`(code, message)`。
async fn config_problems(client: &mut Client) -> Vec<(String, String)> {
    let reply = call(client, "check", json!({})).await;
    reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| problem["kind"] == "config")
        .map(|problem| {
            (
                problem["code"].as_str().unwrap_or_default().to_string(),
                problem["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn system(home: &Home) -> String {
    std::fs::read_to_string(home.root.config_file()).unwrap_or_default()
}

fn resources() -> PathBuf {
    default_resources()
}

#[tokio::test]
async fn removed_settings_stay_quietly_and_come_back_on_reinstall() {
    let home = Home::new();
    home.write("etc/config.toml", "# 留着\nxcfg.port = 9000\n");
    let core = started(&home, &resources());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let folder = source(&home, "one", None);
    let installed = call(&mut client, "package.install", json!({"path": folder})).await;
    assert_eq!(installed["result"]["package"], "xcfg", "{installed}");
    let got = call(&mut client, "config.get", json!({"keys": ["xcfg.port"]})).await;
    assert_eq!(got["result"]["items"]["xcfg.port"]["value"], 9000, "{got}");

    // 看一眼：留下的说几项，顶上的 `settings`（一并删掉的）是空的。
    let preview = call(
        &mut client,
        "package.remove",
        json!({"package": "xcfg", "preview": true}),
    )
    .await;
    assert_eq!(preview["result"]["settings"], json!([]), "{preview}");
    assert_eq!(
        preview["result"]["config"],
        json!({"kept": true, "settings": 1}),
        "配置目录空的不写 path：{preview}"
    );
    let removed = call(&mut client, "package.remove", json!({"package": "xcfg"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert!(
        system(&home).contains("xcfg.port = 9000"),
        "原样留着：{}",
        system(&home)
    );
    let mark = home.root.leftover_settings("xcfg");
    assert_eq!(
        std::fs::read_to_string(&mark).expect("记了"),
        "prefix = \"xcfg\"\n"
    );
    let got = call(&mut client, "config.get", json!({"keys": ["xcfg.port"]})).await;
    assert_eq!(reason(&got), Some("unknown_config_key"), "不生效：{got}");
    let problems = config_problems(&mut client).await;
    assert_eq!(problems.len(), 1, "只说一句：{problems:?}");
    assert_eq!(problems[0].0, "settings_left");
    assert!(
        problems[0].1.contains("1 项")
            && problems[0].1.contains("xcfg")
            && problems[0].1.contains("--purge"),
        "{problems:?}"
    );

    // 重启：照记号认，照样不报不认识。
    let mut again = Client::connect(started(&home, &resources()));
    again.hello().await;
    let problems = config_problems(&mut again).await;
    assert_eq!(
        problems
            .iter()
            .map(|(code, _)| code.as_str())
            .collect::<Vec<_>>(),
        ["settings_left"],
        "{problems:?}"
    );

    // 重装同编号：照原样生效，记号删掉，不再提示。
    let installed = call(&mut client, "package.install", json!({"path": folder})).await;
    assert_eq!(installed["result"]["package"], "xcfg", "{installed}");
    let got = call(&mut client, "config.get", json!({"keys": ["xcfg.port"]})).await;
    assert_eq!(got["result"]["items"]["xcfg.port"]["value"], 9000, "{got}");
    assert!(!mark.exists(), "重装删掉记号");
    assert!(config_problems(&mut client).await.is_empty());

    // `purge` 的照旧删，不留记号。
    let purged = call(
        &mut client,
        "package.remove",
        json!({"package": "xcfg", "purge": true}),
    )
    .await;
    assert_eq!(purged["result"]["removed"], true, "{purged}");
    assert!(!system(&home).contains("xcfg"), "{}", system(&home));
    assert!(system(&home).contains("# 留着"));
    assert!(!mark.exists());
    assert!(config_problems(&mut client).await.is_empty());
}

#[tokio::test]
async fn a_custom_prefix_is_remembered_and_a_new_manifest_wins() {
    let home = Home::new();
    home.write("etc/config.toml", "xc.port = 9000\n");
    let core = started(&home, &resources());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let folder = source(&home, "one", Some("xc"));
    call(&mut client, "package.install", json!({"path": folder})).await;
    let got = call(&mut client, "config.get", json!({"keys": ["xc.port"]})).await;
    assert_eq!(got["result"]["items"]["xc.port"]["value"], 9000, "{got}");
    call(&mut client, "package.remove", json!({"package": "xcfg"})).await;
    assert_eq!(
        std::fs::read_to_string(home.root.leftover_settings("xcfg")).expect("记了"),
        "prefix = \"xc\"\n"
    );
    let problems = config_problems(&mut client).await;
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].0, "settings_left");

    // 重装的清单换了前缀：以新清单为准，旧前缀下的照不认识报。
    let renamed = source(&home, "two", Some("xd"));
    let installed = call(&mut client, "package.install", json!({"path": renamed})).await;
    assert_eq!(installed["result"]["package"], "xcfg", "{installed}");
    assert!(!home.root.leftover_settings("xcfg").exists());
    let problems = config_problems(&mut client).await;
    assert_eq!(
        problems
            .iter()
            .map(|(code, _)| code.as_str())
            .collect::<Vec<_>>(),
        ["unknown_key"],
        "{problems:?}"
    );
}

#[tokio::test]
async fn purging_what_a_removed_package_left() {
    let home = Home::new();
    home.write(
        "etc/config.toml",
        "ui.language = \"en\"\n\n[xcfg]\nport = 9000\n",
    );
    let core = started(&home, &resources());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let folder = source(&home, "one", None);
    call(&mut client, "package.install", json!({"path": folder})).await;
    let config = home.root.package_config("xcfg");
    home.write("etc/xcfg/settings.toml", "volume = 3\n");
    let preview = call(
        &mut client,
        "package.remove",
        json!({"package": "xcfg", "preview": true}),
    )
    .await;
    assert_eq!(
        preview["result"]["config"],
        json!({"path": config.to_string_lossy(), "kept": true, "settings": 1}),
        "{preview}"
    );
    call(&mut client, "package.remove", json!({"package": "xcfg"})).await;

    // 没装的不带 `purge`：照旧不认识。
    for preview in [false, true] {
        let reply = call(
            &mut client,
            "package.remove",
            json!({"package": "xcfg", "preview": preview}),
        )
        .await;
        assert_eq!(reason(&reply), Some("unknown_package"), "{reply}");
    }
    // 带 `purge` 的看一眼：说会删什么，什么都不动。
    let preview = call(
        &mut client,
        "package.remove",
        json!({"package": "xcfg", "purge": true, "preview": true}),
    )
    .await;
    assert_eq!(
        preview["result"],
        json!({"package": "xcfg", "settings": ["xcfg.port"],
               "config": {"path": config.to_string_lossy(), "kept": false, "settings": 1}}),
        "{preview}"
    );
    assert!(system(&home).contains("port = 9000") && config.exists());
    let purged = call(
        &mut client,
        "package.remove",
        json!({"package": "xcfg", "purge": true}),
    )
    .await;
    assert_eq!(
        purged["result"],
        json!({"package": "xcfg", "removed": true}),
        "{purged}"
    );
    assert!(!system(&home).contains("port"), "{}", system(&home));
    assert!(system(&home).contains("ui.language = \"en\""));
    assert!(!config.exists() && !home.root.leftover_settings("xcfg").exists());
    let problems = config_problems(&mut client).await;
    assert!(problems.is_empty(), "{problems:?}");
    // 什么都没留了：照旧不认识。
    let again = call(
        &mut client,
        "package.remove",
        json!({"package": "xcfg", "purge": true}),
    )
    .await;
    assert_eq!(reason(&again), Some("unknown_package"), "{again}");
}

#[tokio::test]
async fn a_removed_shipped_package_is_known_by_its_manifest() {
    let home = Home::new();
    home.write("etc/config.toml", "[courier]\nlisten = 9999\n");
    let resources = shipped::resources(&home);
    let mut client = Client::connect(started(&home, &resources));
    client.hello().await;
    let removed = call(&mut client, "package.remove", json!({"package": "courier"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert!(system(&home).contains("listen = 9999"));
    assert!(
        !home.root.leftover_settings("courier").exists(),
        "出厂的不记：清单还在"
    );
    let problems = config_problems(&mut client).await;
    assert_eq!(
        problems
            .iter()
            .map(|(code, _)| code.as_str())
            .collect::<Vec<_>>(),
        ["settings_left"],
        "{problems:?}"
    );
    // 重启照资源目录里的清单认。
    let mut again = Client::connect(started(&home, &resources));
    again.hello().await;
    assert_eq!(config_problems(&mut again).await.len(), 1);
    // 装回来：照它自己的清单认，不再说是留下的（夹具的程序不在，配置项不进清单，这里不看生效）。
    let back = call(&mut again, "package.install", json!({"package": "courier"})).await;
    assert_eq!(back["result"]["package"], "courier", "{back}");
    let problems = config_problems(&mut again).await;
    assert!(
        problems.iter().all(|(code, _)| code != "settings_left"),
        "{problems:?}"
    );
}

/// 下面一层有同编号的不用记号（施工 PK-5补，sub1 第 2 条）：系统加装的 `xcfg` 被你装的盖住，卸掉你装的那一份不写
/// `etc/leftover/`，系统加装的顶上来，配置里的键照它的清单认、照原样生效，`check` 什么都不说。
#[tokio::test]
async fn removing_a_copy_that_covers_a_lower_one_needs_no_mark() {
    let home = Home::new();
    home.write("etc/config.toml", "xcfg.port = 9000\n");
    let distro = home.work.join("distro");
    std::fs::create_dir_all(distro.join("xcfg")).expect("建得了");
    std::fs::write(distro.join("xcfg/package.toml"), manifest(None)).expect("写得进");
    let core = started_with(&home, &resources(), vec![distro.clone()]);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let folder = source(&home, "one", None);
    let installed = call(&mut client, "package.install", json!({"path": folder})).await;
    assert_eq!(installed["result"]["package"], "xcfg", "{installed}");
    assert!(
        settings_left::lower_has(&core.system_packages(), "xcfg"),
        "系统加装的那一份还在清单里，被盖住"
    );
    let removed = call(&mut client, "package.remove", json!({"package": "xcfg"})).await;
    assert_eq!(removed["result"]["layer"], "home", "{removed}");
    assert!(!home.root.leftover_settings("xcfg").exists(), "不写记号");
    let got = call(&mut client, "config.get", json!({"keys": ["xcfg.port"]})).await;
    assert_eq!(got["result"]["items"]["xcfg.port"]["value"], 9000, "{got}");
    assert!(config_problems(&mut client).await.is_empty());
    assert!(!settings_left::lower_has(&core.system_packages(), "nope"));
}

//! 卸就是清干净（施工 F-8 中下补，设计 `31-软件包.md` 第三节第 3 条、定了的 H）：真核心走一遍。卸掉的包，配置
//! 里它的几项删掉，别的项、注释不动；各层它的 `<编号>/`（设计 55 第三节第 4 条）、脚本的状态和缓存、`provide/` 那一份整个
//! 删掉。家目录里装的、出厂的都一样。开关的文件里它开没开、批过的能力也清掉
//! （施工 F-8 五补）：装回来从头开、从头批。配置目录 `etc/<编号>/` 照 pacman 留下、重装照旧读，`purge` 的才删（施工 PK-5）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::extensions::{
    Program, install, install_asking, quick, read, record, status, steps, until_state,
};
use crate::support::*;

/// 一份带配置项的界面包：`xcfg.port` 写在系统配置里。
const XCFG: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Cfg\" }\n\n[ui]\n\n[settings.port]\ntype = \"int\"\ndefault = 8400\nlayers = [\"system\"]\nname = { en = \"Port\" }\n";

async fn call(client: &mut Client, method: &str, params: Value) -> Value {
    client.call("c", method, params).await
}

#[tokio::test]
async fn removing_a_package_forgets_its_settings_and_state() {
    let home = Home::new();
    write(&home.root.packages().join("xcfg/package.toml"), XCFG);
    write(
        &home.root.config_file(),
        "# 留着\nlog.level = \"debug\"\n\n[xcfg]\nport = 9000\n",
    );
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let got = call(&mut client, "config.get", json!({"keys": ["xcfg.port"]})).await;
    assert_eq!(got["result"]["items"]["xcfg.port"]["value"], 9000, "{got}");
    // 它在各层的 `<编号>/`：配置、状态、缓存、日志、暂存、运行时（设计 55 第三节第 4 条）。
    let places = home.root.package_places("xcfg");
    for place in &places {
        write(&place.join("data"), "x");
    }
    let provided = home.root.provide_cache("xcfg");
    write(&provided, "{}");
    // 它的脚本的状态、缓存（各场所的也算）一起删，`~home` 脚本的、别的包的不动（施工 X-44）。
    let trees = [home.root.script_states(), home.root.script_caches()];
    for tree in &trees {
        for script in [
            "xcfg/live",
            "@qq_3agroup_3a1/xcfg/watch",
            "~home/dice",
            "kit/x",
        ] {
            write(&tree.join(script).join("data"), "x");
        }
    }
    let removed = call(&mut client, "package.remove", json!({"package": "xcfg"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    // 配置里它的设置项照 pacman 留着（施工 PK-5补），`purge` 的才删（`packages_settings_left.rs`）。
    let system = std::fs::read_to_string(home.root.config_file()).unwrap_or_default();
    assert!(
        system.contains("[xcfg]\nport = 9000"),
        "设置项原样留着：{system}"
    );
    assert!(
        system.contains("# 留着") && system.contains("log.level = \"debug\""),
        "别的不动：{system}"
    );
    // 配置目录照 pacman 留下（施工 PK-5），别的几处删了。
    let config = home.root.package_config("xcfg");
    for place in &places {
        let kept = *place == config;
        assert_eq!(place.join("data").exists(), kept, "{}", place.display());
        assert_eq!(place.exists(), kept, "{}", place.display());
    }
    assert!(!provided.exists(), "provide 那一份删了");
    for tree in &trees {
        assert!(!tree.join("xcfg").exists() && !tree.join("@qq_3agroup_3a1/xcfg").exists());
        assert!(tree.join("~home/dice/data").exists() && tree.join("kit/x/data").exists());
    }
}

/// 写一份文件，上一层没有的建上。
fn write(path: &std::path::Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
    std::fs::write(path, text).expect("写得进");
}

#[tokio::test]
async fn removing_a_shipped_package_forgets_its_settings_too() {
    // 出厂的扩展用测试自己放的夹具（施工 S-6 下：接入QQ 搬出了核心仓库）。
    let home = Home::new();
    write(
        &home.root.config_file(),
        "[courier]\nlisten = 9999\n\n[log]\nlevel = \"debug\"\n",
    );
    let resources = shipped::resources(&home);
    let mut client = Client::connect(Arc::new(shipped::core(&home, &resources, &Script::new([]))));
    client.hello().await;
    let state = home.root.package_state("courier");
    std::fs::create_dir_all(&state).expect("建得了");
    // 拉起前建好、什么都没写过的配置目录（施工 PK-5）：没东西可留，卸的时候删掉，不留空目录。
    let config = home.root.package_config("courier");
    std::fs::create_dir_all(&config).expect("建得了");
    let removed = call(&mut client, "package.remove", json!({"package": "courier"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let system = std::fs::read_to_string(home.root.config_file()).unwrap_or_default();
    assert!(
        system.contains("listen = 9999"),
        "设置项留着（施工 PK-5补）：{system}"
    );
    assert!(system.contains("level = \"debug\""), "{system}");
    assert!(!state.exists());
    assert!(!config.exists(), "空的配置目录不留");
}

/// 照 pacman 卸包留配置（施工 PK-5，2026-11-05 项目主人定）：不 `purge` 的 `etc/<编号>/` 原样留下，状态目录照删；留下的
/// 目录 `package.list` 不列、`check` 不报；重装同编号，扩展拿到的 `MIYU_CONFIG_DIR` 还是它、读得到留下的；`purge` 的一起删。
#[tokio::test]
async fn removing_keeps_the_config_until_purged() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "echo");
    let settings = home.root.package_config("echo").join("settings.toml");
    let looked = format!("exists:{}", settings.display());
    let args = [
        keep.as_str(),
        "env:MIYU_CONFIG_DIR",
        &looked,
        "hello",
        "wait",
    ];
    install(&home, "echo", &program.name(), "manual", &steps(&args));
    write(&settings, "volume = 3\n");
    let state = home.root.package_state("echo");
    write(&state.join("data"), "x");
    let source = home.work.join("again").join("echo");
    write(
        &source.join("package.toml"),
        &std::fs::read_to_string(home.root.packages().join("echo/package.toml")).expect("读得到"),
    );
    let core = crate::support::extensions::core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let ran = |times: usize| {
        let path = path.clone();
        async move {
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
            // 一回四行：工作目录、`MIYU_CONFIG_DIR`、配置在不在、握手的回应。
            while read(&path).lines().count() < times * 4 {
                assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            read(&path)
        }
    };
    let on = call(&mut client, "extension.enable", json!({"package": "echo"})).await;
    assert_eq!(on["result"]["on"], true, "{on}");
    ran(1).await;

    let removed = call(&mut client, "package.remove", json!({"package": "echo"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert_eq!(
        std::fs::read_to_string(&settings).ok().as_deref(),
        Some("volume = 3\n"),
        "配置留下、原样"
    );
    assert!(!state.exists(), "状态目录照删");
    let listed = call(&mut client, "package.list", json!({})).await;
    let ids: Vec<&str> = listed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .filter_map(|one| one["package"].as_str())
        .collect();
    assert!(!ids.contains(&"echo"), "留下的配置不算装着：{ids:?}");
    let checked = call(&mut client, "check", json!({})).await;
    let problems = &checked["result"]["problems"];
    assert!(
        problems.is_array() && !problems.to_string().contains("echo"),
        "留下的配置不报：{checked}"
    );

    let back = call(
        &mut client,
        "package.install",
        json!({"path": source.to_string_lossy()}),
    )
    .await;
    assert_eq!(back["result"]["package"], "echo", "{back}");
    let on = call(&mut client, "extension.enable", json!({"package": "echo"})).await;
    assert_eq!(on["result"]["on"], true, "{on}");
    let recorded = ran(2).await;
    let config =
        miyu_fs::usable(&std::fs::canonicalize(home.root.package_config("echo")).expect("在"));
    let again: Vec<&str> = recorded.lines().skip(5).take(2).collect();
    assert_eq!(
        again,
        [
            format!("MIYU_CONFIG_DIR={}", config.display()),
            "exists:true".to_string()
        ],
        "重装同编号照旧读留下的配置：{recorded}"
    );

    let purged = call(
        &mut client,
        "package.remove",
        json!({"package": "echo", "purge": true}),
    )
    .await;
    assert_eq!(purged["result"]["removed"], true, "{purged}");
    assert!(!home.root.package_config("echo").exists(), "purge 的一起删");
    core.stop_extensions().await;
}

#[tokio::test]
async fn removing_an_extension_forgets_its_switch_and_approvals() {
    let home = Home::new();
    let program = Program::new();
    let waits = steps(&["hello", "wait"]);
    install_asking(
        &home,
        "echo",
        &program.name(),
        "manual",
        &waits,
        &["network"],
    );
    let source = home.work.join("again").join("echo");
    std::fs::create_dir_all(&source).expect("建得了");
    std::fs::copy(
        home.root.packages().join("echo/package.toml"),
        source.join("package.toml"),
    )
    .expect("拷得了");
    let core = crate::support::extensions::core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let on = call(
        &mut client,
        "extension.enable",
        json!({"package": "echo", "approve": ["network"]}),
    )
    .await;
    assert_eq!(on["result"]["on"], true, "{on}");
    until_state(&mut client, "echo", |one| one["state"] == "running").await;
    let switches = || std::fs::read_to_string(home.root.extensions_file()).unwrap_or_default();
    assert!(switches().contains("echo"), "{}", switches());
    // 批过的能力记在开关文件里（施工 SU-2 中）。
    assert!(
        switches().contains("\"approved\":{\"echo\":[\"network\"]}"),
        "{}",
        switches()
    );

    let removed = call(&mut client, "package.remove", json!({"package": "echo"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert!(
        !switches().contains("echo"),
        "开没开、批过的一起清掉：{}",
        switches()
    );
    let back = call(
        &mut client,
        "package.install",
        json!({"path": source.to_string_lossy()}),
    )
    .await;
    assert_eq!(back["result"]["enabled"], false, "装回来是关着的：{back}");
    let fresh = status(&mut client, "echo").await;
    assert_eq!(fresh["unapproved"], json!(["network"]), "要重新批：{fresh}");
    core.stop_extensions().await;
}

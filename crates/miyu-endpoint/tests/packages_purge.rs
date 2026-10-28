//! 卸就是清干净（施工 F-8 中下补，设计 `31-软件包.md` 第三节第 3 条、定了的 H）：真核心走一遍。卸掉的包，系统配置、个人设置
//! 里它的几项删掉，别的项、注释不动；状态目录整个删掉。家目录里装的、出厂的都一样。开关的文件里它开没开、批过的能力也清掉
//! （施工 F-8 五补）：装回来从头开、从头批。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::extensions::{Program, install_asking, quick, status, steps, until_state};
use crate::support::*;

/// 一份带配置项的界面包：`xcfg.port` 两层都能写。
const XCFG: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Cfg\" }\n\n[ui]\n\n[settings.port]\ntype = \"int\"\ndefault = 8400\nlayers = [\"system\", \"personal\"]\nname = { en = \"Port\" }\n";

async fn call(client: &mut Client, method: &str, params: Value) -> Value {
    client.call("c", method, params).await
}

fn read(home: &Home, relative: &str) -> String {
    std::fs::read_to_string(home.root.path().join(relative)).unwrap_or_default()
}

#[tokio::test]
async fn removing_a_package_forgets_its_settings_and_state() {
    let home = Home::new();
    home.write("system/packages/xcfg/package.toml", XCFG);
    home.write(
        "system/config.toml",
        "# 留着\nlog.level = \"debug\"\n\n[xcfg]\nport = 9000\n",
    );
    home.write("home/alice/settings.toml", "[xcfg]\nport = 9001\n");
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let got = call(&mut client, "config.get", json!({"keys": ["xcfg.port"]})).await;
    assert_eq!(got["result"]["items"]["xcfg.port"]["value"], 9001, "{got}");
    let state = home.root.path().join("state/packages/xcfg");
    std::fs::create_dir_all(&state).expect("建得了");
    std::fs::write(state.join("data"), "x").expect("写得进");
    // 它的脚本在各账号下的状态目录一起删，家目录里的脚本的、别的包的不动（施工 X-44）。
    for script in [
        "alice/xcfg/live",
        "xbot/xcfg/watch",
        "alice/~home/dice",
        "alice/kit/x",
    ] {
        home.write(&format!("state/scripts/{script}/data"), "x");
    }
    let removed = call(&mut client, "package.remove", json!({"package": "xcfg"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let system = read(&home, "system/config.toml");
    assert!(
        !system.contains("port") && !system.contains("[xcfg]"),
        "系统配置里那一项删了，空了的表头也删了：{system}"
    );
    assert!(
        system.contains("# 留着") && system.contains("log.level = \"debug\""),
        "别的不动：{system}"
    );
    let personal = read(&home, "home/alice/settings.toml");
    assert!(!personal.contains("port"), "个人设置里的也删了：{personal}");
    assert!(!state.exists(), "状态目录删了");
    let scripts = home.root.path().join("state/scripts");
    assert!(!scripts.join("alice/xcfg").exists() && !scripts.join("xbot/xcfg").exists());
    assert!(
        scripts.join("alice/~home/dice/data").exists() && scripts.join("alice/kit/x/data").exists()
    );
}

#[tokio::test]
async fn removing_a_shipped_package_forgets_its_settings_too() {
    // 出厂的扩展用测试自己放的夹具（施工 S-6 下：接入QQ 搬出了核心仓库）。
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[courier]\nlisten = 9999\n\n[log]\nlevel = \"debug\"\n",
    );
    let resources = shipped::resources(&home);
    let mut client = Client::connect(Arc::new(shipped::core(&home, &resources, &Script::new([]))));
    client.hello().await;
    let state = home.root.path().join("state/packages/courier");
    std::fs::create_dir_all(&state).expect("建得了");
    let removed = call(&mut client, "package.remove", json!({"package": "courier"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let system = read(&home, "system/config.toml");
    assert!(!system.contains("listen"), "{system}");
    assert!(system.contains("level = \"debug\""), "{system}");
    assert!(!state.exists());
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
        home.root.path().join("system/packages/echo/package.toml"),
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
    let switches = || read(&home, "system/extensions.json");
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

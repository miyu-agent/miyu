//! 包之间的依赖（施工 F-11，`docs/blueprint/packages.md`「依赖」）：真核心走一遍。`[depends] packages` 写别的包，什么种类都行；
//! 缺了的装不上；被依赖着的卸不掉；装着的缺了依赖的列表里写明、`miyu check` 报出来；以前写的 `workers` 照认；列表带 `[export]`。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 只有 `[package]` 的包（什么程序都不带），依赖、推荐照写的 TOML 段。
fn manifest(name: &str, links: &str) -> String {
    format!("[package]\nname = {{ en = \"{name}\" }}\n{links}")
}

/// 在工作目录里放一个要装的包，交回文件夹的路径。
fn source(home: &Home, id: &str, text: &str) -> PathBuf {
    let folder = home.work.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), text).expect("写得进");
    folder
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

async fn install(client: &mut Client, folder: &Path) -> Value {
    client
        .call(
            "i",
            "package.install",
            json!({"path": folder.to_string_lossy()}),
        )
        .await
}

async fn remove(client: &mut Client, id: &str) -> Value {
    client
        .call("r", "package.remove", json!({"package": id}))
        .await
}

async fn listed(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn a_package_with_missing_depends_is_refused_and_a_depended_one_stays() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let addon = source(
        &home,
        "addon",
        &manifest("Addon", "\n[depends]\npackages = [\"base\", \"extra\"]\n"),
    );
    let refused = install(&mut client, &addon).await;
    assert_eq!(
        refused["error"]["data"]["reason"], "depends_missing",
        "{refused}"
    );
    assert_eq!(
        refused["error"]["data"]["missing"],
        json!(["base", "extra"])
    );
    assert_eq!(
        listed(&mut client, "addon").await,
        Value::Null,
        "什么都没装"
    );

    for id in ["base", "extra"] {
        let reply = install(&mut client, &source(&home, id, &manifest(id, ""))).await;
        assert!(reply.get("error").is_none(), "{reply}");
    }
    let installed = install(&mut client, &addon).await;
    assert!(installed.get("error").is_none(), "{installed}");
    assert_eq!(
        listed(&mut client, "addon").await["depends"],
        json!({"packages": ["base", "extra"]})
    );

    let depended = remove(&mut client, "base").await;
    assert_eq!(
        depended["error"]["data"]["reason"], "package_depended",
        "{depended}"
    );
    assert_eq!(depended["error"]["data"]["by"], json!(["addon"]));
    assert!(remove(&mut client, "addon").await.get("error").is_none());
    let gone = remove(&mut client, "base").await;
    assert!(gone.get("error").is_none(), "没人依赖了照卸：{gone}");
}

#[tokio::test]
async fn an_installed_package_missing_its_depends_says_so_and_old_workers_still_read() {
    let home = Home::new();
    home.write(
        "system/packages/addon/package.toml",
        &manifest(
            "Addon",
            "\n[depends]\nworkers = [\"base\"]\n\n[recommends]\npackages = [\"nice\"]\n",
        ),
    );
    home.write(
        "system/packages/relay/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Relay\" }\n\n[command]\nname = \"relay\"\nprogram = \"miyu-relay\"\nabout = { en = \"R\" }\n\n[process]\n\n[export]\nmethods = [\"qq.inbound\", \"qq.groups\"]\n",
    );
    let mut client = connected(&home).await;
    let addon = listed(&mut client, "addon").await;
    assert_eq!(
        addon["depends"],
        json!({"packages": ["base"]}),
        "以前写的 workers 照认"
    );
    assert_eq!(addon["recommends"], json!({"packages": ["nice"]}));
    assert_eq!(addon["status"], "depends_missing", "{addon}");
    assert_eq!(addon["missing"], json!(["base"]), "推荐的缺了不算");
    let relay = listed(&mut client, "relay").await;
    assert_eq!(
        relay["export"],
        json!({"methods": ["qq.inbound", "qq.groups"]}),
        "{relay}"
    );

    let reply = client.call("k", "check", json!({})).await;
    let problems: Vec<&Value> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| problem["code"] == "depends_missing")
        .collect();
    assert_eq!(problems.len(), 1, "{reply}");
    assert_eq!(problems[0]["file"], "system/packages/addon/package.toml");
    assert_eq!(problems[0]["message"], "缺少依赖的软件包：base");
}

/// 依赖缺了的扩展不拉起：同一批开着的另一个起来了，它还没起，列表写明缺谁。
#[tokio::test]
async fn an_extension_missing_its_depends_is_not_started() {
    use crate::support::extensions::{Program, core, install, quick, until_state};
    let home = Home::new();
    let program = Program::new();
    let args: Vec<String> = ["hello", "serve"].iter().map(ToString::to_string).collect();
    install(&home, "ctrl", &program.name(), "always", &args);
    install(&home, "relay", &program.name(), "always", &args);
    let relay = home.root.path().join("system/packages/relay/package.toml");
    let text = std::fs::read_to_string(&relay).expect("装好了");
    std::fs::write(
        &relay,
        format!("{text}\n[depends]\npackages = [\"base\"]\n"),
    )
    .expect("写得进");
    let core = core(&home, quick());
    let mut client = Client::connect(core);
    client.hello().await;
    until_state(&mut client, "ctrl", |one| one["state"] == "running").await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let reply = client.call("s", "extension.status", json!({})).await;
    let relay_state = reply["result"]["extensions"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["package"] == "relay")
        .map(|one| one["state"].clone());
    assert_ne!(relay_state, Some(json!("running")), "{reply}");
    let listed = listed(&mut client, "relay").await;
    assert_eq!(listed["status"], "depends_missing", "{listed}");
}

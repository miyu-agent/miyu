//! 软件包带的人格、预设（施工 P-11 上，设计 `31-软件包.md` 第十节）：真核心走一遍。装上就列得出、开得了会话；改了写进家目录；
//! 卸了包里的没了，家目录的改动留着；和出厂的同名的不认，`miyu check` 说被谁盖住。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 在工作目录里放一个要装的包：只有 `[package]`，`files` 是包文件夹里的几份（相对位置、内容）。交回文件夹的路径。
fn source(home: &Home, id: &str, files: &[(&str, &str)]) -> PathBuf {
    let folder = home.work.join(id);
    for (relative, text) in
        std::iter::once(&("package.toml", "[package]\nname = { en = \"Kitty\" }\n")).chain(files)
    {
        let path = folder.join(relative);
        std::fs::create_dir_all(path.parent().expect("有上级")).expect("建得了");
        std::fs::write(path, text).expect("写得进");
    }
    folder
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// 列表里编号是 `id` 的那一项；没有的是 `null`。
fn one(listed: &Value, key: &str, id: &str) -> Value {
    listed["result"][format!("{key}s")]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|item| item[key] == id)
        .cloned()
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn a_package_brings_personas_and_presets_and_leaves_your_edits_behind() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let folder = source(
        &home,
        "kitty",
        &[
            (
                "personas/cat/persona.toml",
                "[persona]\nname = \"Cat\"\nsummary = \"A cat.\"\n",
            ),
            ("personas/cat/prompts/persona.md", "You are a cat.\n"),
            ("presets/cozy.toml", "[preset]\nname = \"Cozy\"\n"),
        ],
    );
    let installed = client
        .call(
            "i",
            "package.install",
            json!({"path": folder.to_string_lossy()}),
        )
        .await;
    assert!(installed["result"].is_object(), "{installed}");

    let personas = client.call("l1", "persona.list", json!({})).await;
    assert_eq!(
        one(&personas, "persona", "cat")["name"],
        "Cat",
        "{personas}"
    );
    let presets = client.call("l2", "preset.list", json!({})).await;
    assert_eq!(one(&presets, "preset", "cozy")["name"], "Cozy", "{presets}");
    let got = client
        .call("g1", "persona.get", json!({"persona": "cat"}))
        .await;
    assert_eq!(
        got["result"]["remove"],
        Value::Null,
        "包里的那一份删不掉：{got}"
    );
    let created = client
        .call(
            "c",
            "session.create",
            json!({"cwd": home.work.to_string_lossy(), "persona": "cat", "preset": "cozy"}),
        )
        .await;
    assert!(created["result"]["session"].is_string(), "{created}");

    // 改包里的人格：写进家目录，删掉这一份是回到包里的样子。
    let saved = client
        .call(
            "s",
            "persona.set",
            json!({"persona": "cat", "changes": [{"key": "persona.summary", "value": "My cat."}]}),
        )
        .await;
    assert_eq!(saved["result"]["remove"], "restore", "{saved}");

    let removed = client
        .call("r", "package.remove", json!({"package": "kitty"}))
        .await;
    assert!(removed["result"].is_object(), "{removed}");
    let personas = client.call("l3", "persona.list", json!({})).await;
    let left = one(&personas, "persona", "cat");
    assert_eq!(left["summary"], "My cat.", "家目录的改动留着：{personas}");
    assert_eq!(left["name"], Value::Null, "包里的名字跟着包走了");
    let presets = client.call("l4", "preset.list", json!({})).await;
    assert_eq!(one(&presets, "preset", "cozy"), Value::Null, "{presets}");
    let gone = client
        .call("g2", "preset.get", json!({"preset": "cozy"}))
        .await;
    assert_eq!(gone["error"]["data"]["reason"], "unknown_preset", "{gone}");
}

/// 和出厂的同名的（核心升级后出厂的新带了同名的才撞得上）：包里那一份不认，`miyu check` 说被谁盖住。
#[tokio::test]
async fn a_package_preset_named_like_a_shipped_one_is_covered() {
    let home = Home::new();
    home.write(
        "home/alice/packages/kitty/package.toml",
        "[package]\nname = { en = \"Kitty\" }\n",
    );
    home.write(
        "home/alice/packages/kitty/presets/full.toml",
        "[preset]\nname = \"Not full\"\n",
    );
    let mut client = connected(&home).await;
    let got = client
        .call("g", "preset.get", json!({"preset": "full"}))
        .await;
    assert_ne!(got["result"]["name"], "Not full", "{got}");
    let reply = client.call("k", "check", json!({})).await;
    let problems: Vec<&Value> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| problem["code"] == "preset_covered")
        .collect();
    assert_eq!(problems.len(), 1, "{reply}");
    assert_eq!(problems[0]["kind"], "preset");
    assert_eq!(
        problems[0]["file"],
        "home/alice/packages/kitty/presets/full.toml"
    );
    assert_eq!(problems[0]["level"], "warning");
}

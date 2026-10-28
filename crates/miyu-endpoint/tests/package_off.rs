//! 把某个包整个关掉（施工 U23 末上）：主人关了 `kit`，她新开的会话、开着的会话下一轮都没有它的脚本工具，技能列表、预设页也
//! 不列。必需的包关不了（`config_invalid`），系统配置里写不进；设置页上每个包那一组有这一项，必需的没有。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::scripts::{core, echo};
use crate::support::*;

/// 新会话会拿到的工具名（`session.tools` 不写会话）。
async fn face(client: &mut Client) -> Vec<String> {
    let reply = client.call("t", "session.tools", json!({})).await;
    names(&reply["result"]["tools"])
}

/// 一串里每一项的 `name`。
fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("{list}"))
        .iter()
        .filter_map(|one| one["name"].as_str().map(str::to_string))
        .collect()
}

/// 把系统配置里的 `key` 写成 `value`。
async fn set(client: &mut Client, n: u8, layer: &str, key: &str, value: bool) -> Value {
    client
        .call(
            &format!("config-9f2c4e1a7b3d5f60-{n}"),
            "config.set",
            json!({"layer": layer, "changes": [{"key": key, "value": value}]}),
        )
        .await
}

/// 第 `n` 次请求发给模型的工具名。
fn sent(script: &Script, n: usize) -> Vec<String> {
    script.requests()[n]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

#[tokio::test]
async fn a_package_switched_off_is_gone() {
    let home = Home::new();
    home.write(
        "packages/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n",
    );
    home.write("packages/kit/scripts/dice.py", &echo("dice"));
    home.write(
        "packages/kit/skills/kit-notes/SKILL.md",
        "---\nname: kit-notes\ndescription: Take notes.\n---\n\nBody.\n",
    );
    home.write(
        "packages/kit2/package.toml",
        "[package]\nname = { en = \"Kit 2\" }\n",
    );
    home.write(
        "packages/kit2/skills/kit2-notes/SKILL.md",
        "---\nname: kit2-notes\ndescription: Take other notes.\n---\n\nBody.\n",
    );
    let script = Arc::new(Script::new([Play::Says("一。"), Play::Says("二。")]));
    let core = core(&home, &script);
    let mut alice = Client::connect(Arc::clone(&core));
    alice.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = alice.create("c1", &cwd).await;
    alice.say("s1", &session, "一").await;
    home.until_turns(&session, 1).await;
    assert!(sent(&script, 0).contains(&"dice".to_string()), "关之前有");
    assert!(face(&mut alice).await.contains(&"dice".to_string()));

    let reply = set(&mut alice, 1, "system", "packages.kit.off", true).await;
    assert!(reply["result"].is_object(), "{reply}");
    alice.say("s2", &session, "二").await;
    home.until_turns(&session, 2).await;
    assert!(
        !sent(&script, 1).contains(&"dice".to_string()),
        "开着的会话下一轮就没有"
    );
    assert!(
        !face(&mut alice).await.contains(&"dice".to_string()),
        "新会话没有"
    );
    let skills = alice.call("k", "skill.list", json!({})).await;
    assert!(
        !names(&skills["result"]["skills"]).contains(&"kit-notes".to_string()),
        "技能列表不列：{skills}"
    );
    assert!(
        names(&skills["result"]["skills"]).contains(&"kit2-notes".to_string()),
        "没关的照列：{skills}"
    );
    // 再关一个：关了的换了，看得到的那一份当场重造。
    let reply = set(&mut alice, 4, "system", "packages.kit2.off", true).await;
    assert!(reply["result"].is_object(), "{reply}");
    let skills = alice.call("k2", "skill.list", json!({})).await;
    assert!(
        !names(&skills["result"]["skills"]).contains(&"kit2-notes".to_string()),
        "再关的也不列：{skills}"
    );
    let preset = alice
        .call("p", "preset.get", json!({"preset": "full"}))
        .await;
    assert!(
        !preset["result"]["features"]
            .as_array()
            .unwrap_or_else(|| panic!("{preset}"))
            .iter()
            .any(|one| one["id"] == "script-dice" || one["id"] == "kit"),
        "预设页不列：{preset}"
    );
    // 施工 BS-1 上中起没有必需的包：原来基础系统的 trash 也关得掉。
    let trash = set(&mut alice, 2, "system", "packages.trash.off", true).await;
    assert!(trash["result"].is_object(), "{trash}");

    // 设置页：每个包那一组有这一项（测试的核心照重读清单那一次拼包的项：装一个别的包）。
    let folder = home.work.join("other");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Other\" }\n",
    )
    .expect("写得进");
    let installed = alice
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "other", "{installed}");
    let schema = alice
        .call("s", "config.schema", json!({"keys": ["packages.kit.off"]}))
        .await;
    let item = &schema["result"]["items"][0];
    assert_eq!(
        (item["page"].clone(), item["group"].clone()),
        (json!("packages"), json!("kit")),
        "{schema}"
    );
    let all = alice.call("a", "config.schema", json!({})).await;
    let keys: Vec<&str> = all["result"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{all}"))
        .iter()
        .filter_map(|one| one["key"].as_str())
        .collect();
    assert!(keys.contains(&"packages.kit.off"), "{keys:?}");
    assert!(
        keys.contains(&"packages.shell.off"),
        "原来基础系统的包也有：{keys:?}"
    );
}

/// 原来基础系统的包关掉（施工 BS-1 上中起没有必需的包）：手改系统配置写成关着的，工具就没了，别的包照旧。
#[tokio::test]
async fn a_former_basesystem_package_written_off_by_hand_is_off() {
    let home = Home::new();
    home.write("config.toml", "[packages.read]\noff = true\n");
    let script = Arc::new(Script::new([]));
    let core = core(&home, &script);
    let mut alice = Client::connect(Arc::clone(&core));
    alice.hello().await;
    let face = face(&mut alice).await;
    assert!(!face.contains(&"read".to_string()), "{face:?}");
}

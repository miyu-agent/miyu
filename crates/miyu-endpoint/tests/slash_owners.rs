//! 斜杠命令照背后那件归属（施工 BS-1 下下，设计 `44-基础系统拆包.md` 第三节第 3 条，项目主人 2026-10-27 定）：`/clear`、`/stop`、
//! `/workspace` 是核心自带的，一直在；`/todo` 跟着 `todowrite` 包，卸掉了就没有这条命令（菜单里不列、敲了「没有这条命令」），
//! 名字空出来，别的包带的同名模板照平常装；装回来又有了。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

fn names(reply: &Value) -> Vec<String> {
    reply["result"]["commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|command| command["name"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn todo_goes_with_its_package_and_the_core_ones_stay() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let removed = client
        .call("r", "package.remove", json!({"package": "todowrite"}))
        .await;
    assert!(removed["result"].is_object(), "{removed}");

    let listed = client
        .call("c", "command.catalog", json!({"session": session}))
        .await;
    let listed = names(&listed);
    assert!(!listed.contains(&"todo".to_string()), "{listed:?}");
    for core in ["clear", "stop", "workspace"] {
        assert!(
            listed.contains(&core.to_string()),
            "核心自带的 {core} 在：{listed:?}"
        );
    }
    let typed = client
        .call(
            "k",
            "command.run",
            json!({"session": session, "text": "/todo"}),
        )
        .await;
    assert_eq!(reason(&typed), Some("unknown_command"), "{typed}");

    // 名字空出来：别的包带的 `/todo` 模板照平常装。
    let folder = home.work.join("sources").join("mytodo");
    std::fs::create_dir_all(folder.join("commands")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Mine\" }\n",
    )
    .expect("写得进");
    std::fs::write(folder.join("commands/todo.md"), "List what is left.\n").expect("写得进");
    let installed = client
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert!(installed["result"].is_object(), "名字空出来了：{installed}");

    // 撞了核心自带的照旧装不上，说是核心自带的。
    let clash = home.work.join("sources").join("myclear");
    std::fs::create_dir_all(clash.join("commands")).expect("建得了");
    std::fs::write(
        clash.join("package.toml"),
        "[package]\nname = { en = \"Clear\" }\n",
    )
    .expect("写得进");
    std::fs::write(clash.join("commands/stop.md"), "Stop.\n").expect("写得进");
    let refused = client
        .call("j", "package.install", json!({"path": clash}))
        .await;
    assert_eq!(
        refused["error"]["data"]["conflicts"],
        json!([{"kind": "slash", "name": "stop", "core": true}]),
        "{refused}"
    );
}

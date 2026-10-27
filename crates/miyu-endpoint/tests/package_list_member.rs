//! 成员读软件包列表（施工 U23-R，U23 末上、末下：成员在软件包页照包开关他自己的）：成员调 `package.list` 拿得到，只有名字、说明、
//! 种类、状态、开没开这几格，这台机器上的路径、程序、MCP 服务的写法不给；机主照旧拿全份；软件包的流成员订不了（那里还推 MCP
//! 服务）。管理员订的流回应里拿全份（施工 U23-R修二）。

use serde_json::json;

use crate::support::login::{local, member};
use crate::support::*;

#[tokio::test]
async fn a_member_lists_packages_without_the_machine_details() {
    let home = Home::new();
    let core = packaged::core(&home);
    let mut bob = member(&core, "bob").await;
    let listed = bob.call("l", "package.list", json!({})).await;
    let items = listed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("成员拿得到：{listed}"))
        .clone();
    let shell = items
        .iter()
        .find(|item| item["package"] == "shell")
        .unwrap_or_else(|| panic!("有出厂的 shell：{listed}"));
    assert_eq!(shell["kind"], "builtin", "{shell}");
    assert!(
        shell.get("name").is_some() && shell.get("features").is_some(),
        "{shell}"
    );
    for item in &items {
        for hidden in [
            "state",
            "pages_dir",
            "command",
            "commands",
            "process",
            "worker",
            "problem",
        ] {
            assert!(item.get(hidden).is_none(), "成员看不到 {hidden}：{item}");
        }
    }

    let mut owner = local(&core).await;
    let full = owner.call("l", "package.list", json!({})).await;
    let shell = full["result"]["packages"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["package"] == "shell"))
        .cloned()
        .unwrap_or_else(|| panic!("{full}"));
    assert!(shell.get("state").is_some(), "机主照旧拿全份：{shell}");
    // 订软件包的流（只给管理员组）回应里的那一份同 `package.list` 给管理员的（施工 U23-R修二：原来照成员的裁了）。
    let streamed = owner
        .call("w", "subscribe", json!({"stream": "packages"}))
        .await;
    let shell = streamed["result"]["packages"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["package"] == "shell"))
        .cloned()
        .unwrap_or_else(|| panic!("{streamed}"));
    assert!(shell.get("state").is_some(), "流里机主也拿全份：{shell}");

    let subscribed = bob
        .call("s", "subscribe", json!({"stream": "packages"}))
        .await;
    assert_eq!(
        subscribed["error"]["data"]["reason"], "not_permitted",
        "{subscribed}"
    );
}

/// 不是管理员组的账号也有扩展（接入QQ 的 `im`，施工 U23-R修）：照 `export`、`connection` 找挂在自己身上的包，成员拿到的列表里
/// 照样有；程序、参数、状态目录照样不给。
#[tokio::test]
async fn a_member_still_sees_what_extensions_export() {
    let home = Home::new();
    home.write(
        "system/packages/relay/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Relay\" }\n\n[command]\nname = \"relay\"\nprogram = \"miyu-relay\"\nabout = { en = \"R\" }\n\n[process]\n\n[connection]\nplatform = \"qq\"\n\n[export]\nmethods = [\"qq.inbound\", \"qq.groups\"]\n",
    );
    let core = packaged::core(&home);
    let mut bob = member(&core, "bob").await;
    let listed = bob.call("l", "package.list", json!({})).await;
    let relay = listed["result"]["packages"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["package"] == "relay"))
        .cloned()
        .unwrap_or_else(|| panic!("{listed}"));
    assert_eq!(
        relay["export"],
        json!({"methods": ["qq.inbound", "qq.groups"]}),
        "{relay}"
    );
    assert_eq!(relay["connection"], json!({"platform": "qq"}), "{relay}");
    for hidden in ["state", "command", "commands", "process"] {
        assert!(relay.get(hidden).is_none(), "成员看不到 {hidden}：{relay}");
    }
}

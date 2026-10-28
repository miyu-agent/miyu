//! 界面块（施工 X-15 上，`docs/blueprint/widgets.md`「清单」）：真核心读清单声明的块，`package.list` 列出来（标题照连接的
//! 语言挑）；扩展能力 `widgets` 的名字、说明照语言给；整页的命令名和内置的同名，`miyu check` 报 `slash_builtin`。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 包 `wx`：要 `widgets` 能力，侧边栏一块（跟着会话）、底栏一块、整页一块（命令名撞了内置的 `/stop`）。
const WX: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Weather\" }\n\n[command]\nname = \"wx\"\nprogram = \"miyu-wx\"\nabout = { en = \"W\" }\n\n[process]\ncapabilities = [\"widgets\"]\n\n[[widget]]\nid = \"today\"\nplace = \"sidebar\"\ntitle = { en = \"Weather\", zh = \"天气\" }\nscope = \"session\"\n\n[[widget]]\nid = \"temp\"\nplace = \"status\"\n\n[[widget]]\nid = \"week\"\nplace = \"page\"\ntitle = { en = \"This week\" }\ncommand = \"stop\"\n";

async fn connected(home: &Home) -> Client {
    home.write("packages/wx/package.toml", WX);
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// 列表里编号是 `id` 的那一项。
fn entry(reply: &Value, list: &str, key: &str, id: &str) -> Value {
    reply["result"][list]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|item| item[key] == id)
        .cloned()
        .unwrap_or_else(|| panic!("没列出 {id}：{reply}"))
}

#[tokio::test]
async fn the_package_list_says_where_a_package_puts_things() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let listed = client.call("l", "package.list", json!({})).await;
    let wx = entry(&listed, "packages", "package", "wx");
    assert_eq!(
        wx["widgets"],
        json!([
            {"id": "today", "place": "sidebar", "scope": "session", "title": "天气"},
            {"id": "temp", "place": "status", "scope": "global"},
            {"id": "week", "place": "page", "scope": "global", "title": "This week", "command": "stop"},
        ]),
        "{wx}"
    );
    let shell = entry(&listed, "packages", "package", "shell");
    assert!(
        shell.get("widgets").is_none(),
        "没声明的不带这一格：{shell}"
    );
}

#[tokio::test]
async fn the_widgets_capability_is_named_in_the_connections_language() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let status = client.call("s", "extension.status", json!({})).await;
    let wx = entry(&status, "extensions", "package", "wx");
    assert_eq!(wx["capabilities"][0]["id"], "widgets", "{wx}");
    assert_eq!(wx["capabilities"][0]["name"], "在界面上显示内容", "{wx}");
    assert_eq!(wx["unapproved"], json!(["widgets"]), "{wx}");
}

#[tokio::test]
async fn check_says_a_page_command_is_taken_by_a_builtin() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let reply = client.call("k", "check", json!({})).await;
    let builtin: Vec<&Value> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|one| one["kind"] == "slash" && one["code"] == "slash_builtin")
        .collect();
    assert_eq!(builtin.len(), 1, "{reply}");
    assert!(
        builtin[0]["file"]
            .as_str()
            .unwrap_or_default()
            .ends_with("wx/package.toml"),
        "{reply}"
    );
    assert!(
        builtin[0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("/stop"),
        "{reply}"
    );
}

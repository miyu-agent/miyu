//! 整页的命令（施工 X-15 下，`docs/blueprint/widgets.md`「整页的命令」）：菜单里列出来、带 `opens`；打了回 `opens`、不碰会话；
//! 自己放的同名模板盖住它；场所会话里不认；关着的扩展的不算。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 包 `wx`：一开就开着（不用真拉起程序），整页 `week` 带命令 `/week`。
const WX: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Weather\" }\n\n[command]\nname = \"wx\"\nprogram = \"miyu-wx\"\nabout = { en = \"W\" }\n\n[process]\nstart = \"always\"\ncapabilities = [\"widgets\"]\n\n[[widget]]\nid = \"week\"\nplace = \"page\"\ntitle = { en = \"This week\", zh = \"这一周\" }\ncommand = \"week\"\n";

fn opens() -> Value {
    json!({"package": "wx", "widget": "week"})
}

/// 菜单里叫 `name` 的那几项。
fn items(reply: &Value, name: &str) -> Vec<Value> {
    reply["result"]["commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|one| one["name"] == name)
        .cloned()
        .collect()
}

#[tokio::test]
async fn the_menu_lists_a_page_command_with_where_it_opens() {
    let home = Home::new();
    home.write("home/alice/packages/wx/package.toml", WX);
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let all = client.call("c1", "command.catalog", json!({})).await;
    assert_eq!(
        items(&all, "week"),
        [json!({"name": "week", "aliases": [], "summary": "这一周", "opens": opens()})],
        "{all}"
    );
    let session = client.create("c2", "~").await;
    let here = client
        .call("c3", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(
        items(&here, "week").len(),
        1,
        "会话里照样列，不归预设管：{here}"
    );
}

#[tokio::test]
async fn typing_it_only_tells_the_head_to_open_the_page() {
    let home = Home::new();
    home.write("home/alice/packages/wx/package.toml", WX);
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let before = home.log(&session).len();
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/week"}),
        )
        .await;
    assert_eq!(
        reply["result"],
        json!({"command": "week", "opens": opens(), "said": ""}),
        "{reply}"
    );
    assert_eq!(home.log(&session).len(), before, "不碰会话、不记日志");
}

#[tokio::test]
async fn ones_own_template_covers_it() {
    let home = Home::new();
    home.write("home/alice/packages/wx/package.toml", WX);
    home.write("home/alice/commands/week.md", "Summarize my week.\n");
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let all = client.call("c1", "command.catalog", json!({})).await;
    let week = items(&all, "week");
    assert_eq!(week.len(), 1, "{all}");
    assert!(week[0].get("opens").is_none(), "列的是自己的模板：{all}");
    let session = client.create("c2", "~").await;
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/week"}),
        )
        .await;
    assert!(reply["result"].get("opens").is_none(), "{reply}");
    assert!(
        reply["result"]["events"].is_array(),
        "照模板发出去了：{reply}"
    );
}

#[tokio::test]
async fn a_switched_off_extension_or_a_venue_does_not_know_it() {
    let home = Home::new();
    home.write(
        "home/alice/packages/wx/package.toml",
        &WX.replace("start = \"always\"", "start = \"manual\""),
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let all = client.call("c1", "command.catalog", json!({})).await;
    assert!(items(&all, "week").is_empty(), "关着的不列：{all}");
    let session = client.create("c2", "~").await;
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/week"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("unknown_command"), "{reply}");

    let home = Home::new();
    home.write("home/alice/packages/wx/package.toml", WX);
    let mut client = Client::connect(bound_core(&home, &Script::new([]), Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let member = json!({"external": "qq:20002", "role": "member"});
    let reply = client
        .call(
            "k2",
            "command.run",
            json!({"session": session, "text": "/week", "as": member}),
        )
        .await;
    assert_eq!(
        reason(&reply),
        Some("unknown_command"),
        "场所里照没有这个命令：{reply}"
    );
}

//! 命令谁能用（施工 O-6 六补，`docs/blueprint/slash.md`「谁能用」，接入QQ 的会话 2026-10-17 要的）：系统配置
//! `commands.<名字>.access` 写 `admin`（照旧：本人、对应表里认得的、管理的人）、`anyone`（谁都行）、`off`（谁都不行，回
//! `command_not_allowed`）。命令自己的限制照旧：`/workspace` 只给主人本人。菜单照它列。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::{BINDINGS, configured_core};
use crate::support::*;

async fn run(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "command.run", params).await
}

/// 菜单里的名字，照先后。
fn names(reply: &Value) -> Vec<String> {
    reply["result"]["commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|command| command["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn access_lets_anyone_in_or_turns_a_command_off() {
    let home = Home::new();
    home.write(
        "config.toml",
        &format!(
            "{BINDINGS}\n[commands.clear]\naccess = \"anyone\"\n\n[commands.stop]\naccess = \"off\"\n\n[commands.workspace]\naccess = \"anyone\"\n\n[commands.todo]\naccess = \"admin\"\n"
        ),
    );
    let script = Script::new([Play::Says("在。")]);
    let mut client = Client::connect(configured_core(&home, &script, Catalog::default()));
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
        .unwrap_or_else(|| panic!("{made}"))
        .to_string();
    let owner = json!({"external": "qq:10001"});
    let member = json!({"external": "qq:20002", "role": "member"});
    let said = json!({"session": session, "text": "在吗", "as": owner});
    client.call("s1", "session.send", said).await;
    home.until_turns(&session, 1).await;

    let reply = run(
        &mut client,
        "k1",
        json!({"session": session, "text": "/clear", "as": member}),
    )
    .await;
    assert_eq!(
        reply["result"]["command"], "clear",
        "anyone：群员也行 {reply}"
    );
    let reply = run(
        &mut client,
        "k2",
        json!({"session": session, "text": "/stop", "as": owner}),
    )
    .await;
    assert_eq!(
        reason(&reply),
        Some("command_not_allowed"),
        "off：主人也不行 {reply}"
    );
    assert_eq!(reply["error"]["data"]["why"], "/stop 已停用。");
    let reply = run(
        &mut client,
        "k3",
        json!({"session": session, "text": "/todo", "as": member}),
    )
    .await;
    assert_eq!(
        reason(&reply),
        Some("command_not_allowed"),
        "admin 照旧 {reply}"
    );
    let reply = run(
        &mut client,
        "k4",
        json!({"session": session, "text": "/workspace ~", "as": member}),
    )
    .await;
    assert_eq!(
        reason(&reply),
        Some("owner_only"),
        "命令自己的限制照旧：只给主人本人 {reply}"
    );

    let local = client.create("c1", "~").await;
    let reply = run(
        &mut client,
        "k5",
        json!({"session": local, "text": "/stop"}),
    )
    .await;
    assert_eq!(
        reason(&reply),
        Some("command_not_allowed"),
        "本机也停用 {reply}"
    );
    let catalog = client
        .call("c2", "command.catalog", json!({"session": local}))
        .await;
    assert!(
        !names(&catalog).contains(&"stop".to_string()),
        "停用的不列：{catalog}"
    );
    assert!(names(&catalog).contains(&"clear".to_string()), "{catalog}");
}

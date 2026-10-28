//! 项目（施工 H-1，设计 `38-项目.md` 第一、二节）：建、列、改、删，各格查写法、人格和预设在不在，`unset` 清掉一格；订阅 `projects`
//! 的回应是整张表，之后别的连接改了的推 `projects.changed`（整项、删了的 `deleted`）；存在家目录的 `projects.json`。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 拒绝的原因码、`data.field`。
fn refused(reply: &Value) -> (&str, &str) {
    (
        reply["error"]["data"]["reason"]
            .as_str()
            .unwrap_or_default(),
        reply["error"]["data"]["field"].as_str().unwrap_or_default(),
    )
}

/// 读到一条 `projects.changed` 为止：别的消息跳过，最多 60 秒。
async fn changed(client: &mut Client) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到项目的推送"));
        if message["method"] == "projects.changed" {
            return message["params"].clone();
        }
    }
}

#[tokio::test]
async fn projects_are_created_listed_updated_and_deleted() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let empty = client.call("l", "project.list", json!({})).await;
    assert_eq!(empty["result"], json!({"projects": []}), "{empty}");

    let created = client
        .call(
            "c",
            "project.create",
            json!({"name": "Miyu", "description": "重制", "color": "#336699", "workspace": "~/src/miyu",
                   "persona": "engineer", "preset": "full", "prompt": "Reply in Chinese.", "open": true}),
        )
        .await;
    let id = created["result"]["project"]
        .as_str()
        .unwrap_or_else(|| panic!("{created}"))
        .to_string();
    let bare = client
        .call(
            "b",
            "project.create",
            json!({"name": "Notes", "persona": null}),
        )
        .await;
    let bare = bare["result"]["project"]
        .as_str()
        .expect("有编号")
        .to_string();
    assert!(bare > id, "编号照造的先后");

    let listed = client.call("l", "project.list", json!({})).await;
    let projects = listed["result"]["projects"].as_array().expect("一串");
    assert_eq!(projects.len(), 2);
    let first = &projects[0];
    assert_eq!(first["project"], id.as_str());
    for (key, want) in [
        ("name", json!("Miyu")),
        ("description", json!("重制")),
        ("color", json!("#336699")),
        ("workspace", json!("~/src/miyu")),
        ("persona", json!("engineer")),
        ("preset", json!("full")),
        ("prompt", json!("Reply in Chinese.")),
        ("open", json!(true)),
    ] {
        assert_eq!(first[key], want, "{key}：{first}");
    }
    assert!(first["created"].is_string());
    assert_eq!(projects[1]["persona"], Value::Null, "明着无人格");
    assert!(projects[1].get("persona").is_some());
    assert!(projects[1].get("color").is_none(), "没写的不写");

    let updated = client
        .call(
            "u",
            "project.update",
            json!({"project": id, "name": "Miyu 2", "unset": ["color", "persona", "prompt"], "workspace": ""}),
        )
        .await;
    assert_eq!(updated["result"], json!({}), "{updated}");
    let listed = client.call("l", "project.list", json!({})).await;
    let first = &listed["result"]["projects"][0];
    assert_eq!(first["name"], "Miyu 2");
    for gone in ["color", "persona", "prompt", "workspace"] {
        assert!(first.get(gone).is_none(), "{gone} 清掉了：{first}");
    }
    assert_eq!(first["description"], "重制", "没碰的照旧");

    let deleted = client
        .call("d", "project.delete", json!({"project": id}))
        .await;
    assert_eq!(deleted["result"], json!({"moved": 0}), "{deleted}");
    let again = client
        .call("d2", "project.delete", json!({"project": id}))
        .await;
    assert_eq!(refused(&again).0, "unknown_project");
    let listed = client.call("l", "project.list", json!({})).await;
    assert_eq!(
        listed["result"]["projects"].as_array().map(Vec::len),
        Some(1)
    );
    let file = home.root.path().join("projects.json");
    let text = std::fs::read_to_string(&file).expect("存在家目录");
    assert!(text.contains(&bare) && !text.contains(&id), "{text}");
}

#[tokio::test]
async fn wrong_fields_are_refused_and_nothing_is_written() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let create = |params: Value| params;
    for (params, want) in [
        (create(json!({})), ("bad_params", "name")),
        (create(json!({"name": ""})), ("bad_params", "name")),
        (
            create(json!({"name": "字".repeat(41)})),
            ("bad_params", "name"),
        ),
        (
            create(json!({"name": "a", "color": "blue"})),
            ("bad_params", "color"),
        ),
        (
            create(json!({"name": "a", "description": "字".repeat(81)})),
            ("bad_params", "description"),
        ),
        (
            create(json!({"name": "a", "prompt": "字".repeat(8001)})),
            ("bad_params", "prompt"),
        ),
        (
            create(json!({"name": "a", "persona": "nobody"})),
            ("unknown_persona", ""),
        ),
        (
            create(json!({"name": "a", "preset": "nothing"})),
            ("unknown_preset", ""),
        ),
    ] {
        let reply = client.call("c", "project.create", params.clone()).await;
        assert_eq!(refused(&reply), want, "{params}：{reply}");
    }
    let file = home.root.path().join("projects.json");
    assert!(!file.exists(), "拒了的什么都不写");

    let made = client
        .call("c", "project.create", json!({"name": "a"}))
        .await;
    let id = made["result"]["project"]
        .as_str()
        .expect("有编号")
        .to_string();
    for (params, want) in [
        (
            json!({"project": id, "unset": ["name"]}),
            ("bad_params", "name"),
        ),
        (
            json!({"project": id, "unset": ["created"]}),
            ("bad_params", "created"),
        ),
        (
            json!({"project": id, "color": "#000000", "unset": ["color"]}),
            ("bad_params", "color"),
        ),
        (
            json!({"project": id, "color": "#00000"}),
            ("bad_params", "color"),
        ),
        (
            json!({"project": "nope", "name": "b"}),
            ("unknown_project", ""),
        ),
    ] {
        let reply = client.call("u", "project.update", params.clone()).await;
        assert_eq!(refused(&reply), want, "{params}：{reply}");
    }
    let listed = client.call("l", "project.list", json!({})).await;
    assert_eq!(listed["result"]["projects"][0]["name"], "a", "拒了的不改");
    assert!(listed["result"]["projects"][0].get("color").is_none());
}

#[tokio::test]
async fn subscribers_get_the_table_then_each_project_that_changed() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut watching = Client::connect(Arc::clone(&core));
    watching.hello().await;
    let mut acting = Client::connect(Arc::clone(&core));
    acting.hello().await;
    acting
        .call("c", "project.create", json!({"name": "Old"}))
        .await;
    let subscribed = watching
        .call("s", "subscribe", json!({"stream": "projects"}))
        .await;
    let table = subscribed["result"]["projects"].as_array().expect("整张表");
    assert_eq!(table.len(), 1, "{subscribed}");

    let made = acting
        .call("c", "project.create", json!({"name": "New"}))
        .await;
    let id = made["result"]["project"]
        .as_str()
        .expect("有编号")
        .to_string();
    let pushed = changed(&mut watching).await;
    assert_eq!(pushed["project"]["project"], id.as_str());
    assert_eq!(pushed["project"]["name"], "New");
    acting
        .call("u", "project.update", json!({"project": id, "open": true}))
        .await;
    assert_eq!(changed(&mut watching).await["project"]["open"], true);
    acting
        .call("d", "project.delete", json!({"project": id}))
        .await;
    assert_eq!(
        changed(&mut watching).await,
        json!({"project": id, "deleted": true})
    );

    watching
        .call("o", "unsubscribe", json!({"stream": "projects"}))
        .await;
    acting
        .call("c", "project.create", json!({"name": "Quiet"}))
        .await;
    let more = watching.next_within(Duration::from_millis(300)).await;
    assert!(more.is_none(), "取消了不推：{more:?}");
}

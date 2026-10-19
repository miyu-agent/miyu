//! 会话在项目里（施工 H-2，设计 `38-项目.md` 第一节第 2、3 条、第二节第 2、4 条）：在项目里建的会话照项目的人格、预设，记一条
//! `session.meta_changed`；`session.set_meta` 移进、移出，没有的项目、分叉出来的会话拒；会话列表每项带 `project`；删项目把里面的
//! 会话挪出来、回挪了几个。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
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

/// 会话列表里编号是 `session` 的那一项。
async fn listed(client: &mut Client, session: &str) -> Value {
    let reply = client.call("l", "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["session"] == session).cloned())
        .unwrap_or_else(|| panic!("列表里有 {session}：{reply}"))
}

/// 建一个项目，交回编号。
async fn project(client: &mut Client, params: Value) -> String {
    let reply = client.call("p", "project.create", params).await;
    reply["result"]["project"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_string()
}

#[tokio::test]
async fn a_session_made_in_a_project_starts_from_its_defaults() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let id = project(
        &mut client,
        json!({"name": "Miyu", "persona": "engineer", "preset": "dev"}),
    )
    .await;
    let made = client
        .call("c", "session.create", json!({"cwd": "~", "project": id}))
        .await;
    assert_eq!(made["result"]["events"], json!([1, 2]), "{made}");
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let item = listed(&mut client, &session).await;
    assert_eq!(
        (&item["project"], &item["persona"], &item["preset"]),
        (&json!(id), &json!("engineer"), &json!("dev")),
        "{item}"
    );
    let log = home.log(&session);
    assert!(
        matches!(&log[1].body, Body::MetaChanged(changed) if changed.project.as_deref() == Some(id.as_str())),
        "{:?}",
        log[1].body
    );

    let mine = client
        .call(
            "c2",
            "session.create",
            json!({"cwd": "~", "project": id, "persona": null}),
        )
        .await;
    let mine = mine["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    assert!(
        listed(&mut client, &mine).await.get("persona").is_none(),
        "明着写的照写的"
    );
    let unknown = client
        .call(
            "c3",
            "session.create",
            json!({"cwd": "~", "project": "nope"}),
        )
        .await;
    assert_eq!(refused(&unknown).0, "unknown_project");
}

#[tokio::test]
async fn sessions_move_in_and_out_and_deleting_moves_them_out() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let id = project(&mut client, json!({"name": "Miyu"})).await;
    let first = client.create("c1", "~").await;
    let second = client.create("c2", "~").await;
    for (n, session) in [&first, &second].into_iter().enumerate() {
        let moved = client
            .call(
                &format!("m{n}"),
                "session.set_meta",
                json!({"session": session, "project": id}),
            )
            .await;
        assert_eq!(moved["result"], json!({}), "{moved}");
    }
    assert_eq!(listed(&mut client, &first).await["project"], id.as_str());

    let out = client
        .call(
            "o",
            "session.set_meta",
            json!({"session": second, "project": null}),
        )
        .await;
    assert_eq!(out["result"], json!({}), "{out}");
    assert!(
        listed(&mut client, &second).await.get("project").is_none(),
        "移出来了"
    );
    for (params, want) in [
        (
            json!({"session": first, "project": "nope"}),
            ("unknown_project", ""),
        ),
        (
            json!({"session": first, "project": ""}),
            ("bad_params", "project"),
        ),
    ] {
        let reply = client.call("x", "session.set_meta", params.clone()).await;
        assert_eq!(refused(&reply), want, "{params}：{reply}");
    }
    let forked = client
        .call("f", "session.fork", json!({"session": second}))
        .await;
    let child = forked["result"]["session"]
        .as_str()
        .expect("分叉出来了")
        .to_string();
    let reply = client
        .call(
            "fc",
            "session.set_meta",
            json!({"session": child, "project": id}),
        )
        .await;
    assert_eq!(
        refused(&reply),
        ("bad_params", "project"),
        "分叉出来的不能进项目"
    );

    client
        .call(
            "m2",
            "session.set_meta",
            json!({"session": second, "project": id}),
        )
        .await;
    let deleted = client
        .call("d", "project.delete", json!({"project": id}))
        .await;
    assert_eq!(deleted["result"], json!({"moved": 2}), "{deleted}");
    for session in [&first, &second] {
        let item = listed(&mut client, session).await;
        assert!(item.get("project").is_none(), "删了项目挪出来：{item}");
    }
    let again = client
        .call("d2", "project.delete", json!({"project": id}))
        .await;
    assert_eq!(refused(&again).0, "unknown_project");
}

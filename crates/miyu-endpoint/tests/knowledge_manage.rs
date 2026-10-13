//! 人管理知识库的协议（施工 K-7 上，`docs/blueprint/knowledge.md`「怎么走」第八条，`protocol.md` 的 `kb.*`）：真核心。建库、
//! 改名字说明、挂目录、加文档（照上传好的 blob）、删文档、重建索引、删库的回应一格一格对；已经有的、挂进来的、挂不上的、没有的
//! 各自的拒绝；参数写错的 `bad_params`。

use std::sync::Arc;

use serde_json::json;

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::*;

async fn client(home: &Home) -> Client {
    let core = home.core_full(
        &Script::new([]),
        Catalog::default(),
        Some(home.work.clone()),
        TOKEN,
    );
    let mut client = Client::connect(Arc::new(core));
    client.hello().await;
    client
}

/// 上传一份 `text`，交回 blob。
async fn upload(client: &mut Client, id: &str, name: &str, text: &str) -> String {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(text);
    let reply = client
        .call(id, "blob.put", json!({"data": data, "name": name}))
        .await;
    reply["result"]["blob"]
        .as_str()
        .expect("传上去了")
        .to_string()
}

#[tokio::test]
async fn creating_renaming_adding_and_reindexing() {
    let home = Home::new();
    let mut client = client(&home).await;
    let created = client
        .call(
            "c1",
            "kb.create",
            json!({"kb": "notes", "name": "笔记", "description": "学日语的"}),
        )
        .await;
    assert_eq!(
        created["result"],
        json!({"id": "notes", "name": "笔记", "description": "学日语的", "documents": 0, "chunks": 0, "auto": false}),
        "{created}"
    );
    let again = client.call("c2", "kb.create", json!({"kb": "notes"})).await;
    assert_eq!(reason(&again), Some("kb_exists"), "{again}");
    let renamed = client
        .call("u1", "kb.update", json!({"kb": "notes", "name": "日语"}))
        .await;
    assert_eq!(renamed["result"]["name"], "日语", "{renamed}");
    assert_eq!(renamed["result"]["description"], "学日语的");
    // 自动想起的开关（施工 K-6 下）：只写 `auto` 也算改了一格，`false` 拿掉。
    let on = client
        .call("u2", "kb.update", json!({"kb": "notes", "auto": true}))
        .await;
    assert_eq!(
        (&on["result"]["auto"], &on["result"]["name"]),
        (&json!(true), &json!("日语")),
        "{on}"
    );
    let off = client
        .call("u3", "kb.update", json!({"kb": "notes", "auto": false}))
        .await;
    assert_eq!(off["result"]["auto"], false, "{off}");

    let blob = upload(&mut client, "b1", "a.md", "团子是橘猫\n").await;
    let added = client
        .call(
            "a1",
            "kb.add",
            json!({"kb": "notes", "path": "cats/a.md", "blob": blob}),
        )
        .await;
    assert_eq!(added["result"], json!({}), "{added}");
    let twice = client
        .call(
            "a2",
            "kb.add",
            json!({"kb": "notes", "path": "cats/a.md", "blob": blob}),
        )
        .await;
    assert_eq!(reason(&twice), Some("document_exists"), "{twice}");
    let newer = upload(&mut client, "b2", "a.md", "团子改名叫小橘\n").await;
    let replaced = client
        .call(
            "a3",
            "kb.add",
            json!({"kb": "notes", "path": "cats/a.md", "blob": newer, "replace": true}),
        )
        .await;
    assert_eq!(replaced["result"], json!({}), "{replaced}");
    let found = client
        .call("s1", "kb.search", json!({"query": "小橘"}))
        .await;
    assert_eq!(found["result"]["hits"][0]["path"], "cats/a.md", "{found}");

    let rebuilt = client
        .call("r1", "kb.reindex", json!({"kb": "notes"}))
        .await;
    assert_eq!(
        (
            &rebuilt["result"]["documents"],
            &rebuilt["result"]["chunks"]
        ),
        (&json!(1), &json!(1)),
        "{rebuilt}"
    );
}

#[tokio::test]
async fn mounting_removing_and_deleting() {
    let home = Home::new();
    let mut client = client(&home).await;
    let notes = home.work.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("a.md"), "团子\n").expect("写得进");
    let mounted = client
        .call("m1", "kb.mount", json!({"kb": "mine", "folder": "~/notes"}))
        .await;
    assert_eq!(
        (&mounted["result"]["id"], &mounted["result"]["documents"]),
        (&json!("mine"), &json!(1)),
        "{mounted}"
    );
    let gone = client
        .call(
            "m2",
            "kb.mount",
            json!({"kb": "gone", "folder": "~/nowhere"}),
        )
        .await;
    assert_eq!(reason(&gone), Some("folder_unavailable"), "{gone}");
    assert!(gone["error"]["data"]["message"].is_string());
    let blob = upload(&mut client, "b1", "b.md", "阿黄\n").await;
    let refused = client
        .call(
            "a1",
            "kb.add",
            json!({"kb": "mine", "path": "b.md", "blob": blob}),
        )
        .await;
    assert_eq!(reason(&refused), Some("kb_read_only"), "{refused}");
    let not_there = client
        .call("a2", "kb.remove", json!({"kb": "mine", "path": "a.md"}))
        .await;
    assert_eq!(reason(&not_there), Some("kb_read_only"), "{not_there}");

    client.call("c1", "kb.create", json!({"kb": "own"})).await;
    client
        .call(
            "a3",
            "kb.add",
            json!({"kb": "own", "path": "b.md", "blob": blob}),
        )
        .await;
    for (id, method, kb) in [
        ("d1", "kb.remove", "own"),
        ("d2", "kb.delete", "own"),
        ("d3", "kb.delete", "mine"),
    ] {
        let params = match method {
            "kb.remove" => json!({"kb": kb, "path": "b.md"}),
            _ => json!({"kb": kb}),
        };
        let reply = client.call(id, method, params).await;
        // 有的机器上临时目录那块盘没有能放的回收站：照实报。
        match reason(&reply) {
            None => assert!(reply["result"]["trash"].is_string(), "{reply}"),
            Some(refused) => assert_eq!(refused, "trash_failed", "{reply}"),
        }
    }
    assert_eq!(
        std::fs::read_to_string(notes.join("a.md")).expect("在"),
        "团子\n",
        "挂的目录没动"
    );
}

#[tokio::test]
async fn wrong_requests_are_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    client.call("c0", "kb.create", json!({"kb": "notes"})).await;
    for (method, params, expected) in [
        ("kb.create", json!({"kb": "笔记"}), "bad_params"),
        (
            "kb.create",
            json!({"kb": "x", "folder": "/tmp"}),
            "bad_params",
        ),
        ("kb.update", json!({"kb": "notes"}), "bad_params"),
        (
            "kb.update",
            json!({"kb": "notes", "auto": "yes"}),
            "bad_params",
        ),
        (
            "kb.update",
            json!({"kb": "nope", "name": "x"}),
            "unknown_kb",
        ),
        ("kb.mount", json!({"kb": "x"}), "bad_params"),
        ("kb.delete", json!({"kb": "nope"}), "unknown_kb"),
        ("kb.reindex", json!({"kb": "nope"}), "unknown_kb"),
        (
            "kb.add",
            json!({"kb": "notes", "path": "a.md"}),
            "bad_params",
        ),
        (
            "kb.add",
            json!({"kb": "notes", "path": "a.exe", "blob": "sha256:00"}),
            "bad_params",
        ),
        (
            "kb.add",
            json!({"kb": "notes", "path": "a.md", "blob": format!("sha256:{}", "0".repeat(64))}),
            "unknown_attachment",
        ),
        (
            "kb.remove",
            json!({"kb": "notes", "path": "../a.md"}),
            "bad_params",
        ),
        (
            "kb.remove",
            json!({"kb": "notes", "path": "gone.md"}),
            "unknown_document",
        ),
        (
            "kb.remove",
            json!({"kb": "notes", "path": "a.md", "replace": true}),
            "bad_params",
        ),
    ] {
        let reply = client.call("w", method, params.clone()).await;
        assert_eq!(reason(&reply), Some(expected), "{method} {params}: {reply}");
    }
}

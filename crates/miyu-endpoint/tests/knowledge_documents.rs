//! `kb.documents`（施工 K-10，`docs/blueprint/knowledge.md`「对外的样子」）：真核心。一篇一项、读不了的带 `reason`、`why`，读不了
//! 的子目录在 `folders`；挂进来的库在 `kb.list` 里带 `mount`；挂的目录用不了的 `folder_unavailable`；参数写错的、没有的库照拒。

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

#[tokio::test]
async fn documents_are_listed_with_what_could_not_be_read() {
    let home = Home::new();
    let notes = home.root.account_dir(&alice()).join("kb").join("notes");
    std::fs::create_dir_all(notes.join("sub")).expect("建得了");
    std::fs::write(notes.join("a.md"), "# 猫\n团子\n").expect("写得进");
    std::fs::write(notes.join("sub/b.txt"), [0xff, 0xfe]).expect("写得进");
    std::fs::write(notes.join("c.png"), "not a document").expect("写得进");
    let mut client = client(&home).await;
    let listed = client
        .call("d1", "kb.documents", json!({"kb": "notes"}))
        .await;
    let result = &listed["result"];
    assert_eq!(result["folders"], json!([]), "{listed}");
    let documents = result["documents"].as_array().expect("一串");
    assert_eq!(documents.len(), 2, "不是文档的不列：{listed}");
    let at = documents[0]["at"].as_str().expect("有时刻").to_string();
    assert_eq!(
        documents[0],
        json!({"path": "a.md", "kind": "markdown", "size": 13, "at": at, "chunks": 1}),
        "读得了的不带 reason、why"
    );
    assert_eq!(
        (
            &documents[1]["path"],
            &documents[1]["kind"],
            &documents[1]["chunks"],
            &documents[1]["reason"]
        ),
        (
            &json!("sub/b.txt"),
            &json!("text"),
            &json!(0),
            &json!("not_utf8")
        )
    );
    assert_eq!(documents[1]["why"], "not UTF-8");
}

#[tokio::test]
async fn a_mount_is_shown_and_an_unplugged_one_is_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    let notes = home.work.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("a.md"), "团子\n").expect("写得进");
    let mounted = client
        .call("m1", "kb.mount", json!({"kb": "mine", "folder": "~/notes"}))
        .await;
    assert_eq!(
        mounted["result"]["mount"], "~/notes",
        "照写的原样：{mounted}"
    );
    let listed = client.call("l1", "kb.list", json!({})).await;
    assert_eq!(listed["result"]["bases"][0]["mount"], "~/notes", "{listed}");
    std::fs::rename(&notes, home.work.join("away")).expect("挪得走");
    let refused = client
        .call("d1", "kb.documents", json!({"kb": "mine"}))
        .await;
    assert_eq!(reason(&refused), Some("folder_unavailable"), "{refused}");
    assert!(refused["error"]["data"]["message"].is_string());
}

#[tokio::test]
async fn wrong_params_and_unknown_bases_are_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    for (n, (params, expected)) in [
        (json!({}), "bad_params"),
        (json!({"kb": "Bad Id"}), "bad_params"),
        (json!({"kb": "notes", "page": 1}), "bad_params"),
        (json!({"kb": "nope"}), "unknown_kb"),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(&format!("w{n}"), "kb.documents", params.clone())
            .await;
        assert_eq!(reason(&reply), Some(expected), "{params} → {reply}");
    }
}

//! `kb.remove` 收文件夹、收几样（施工 W-14，网页的知识库页按文件夹删、多选删）：`path` 是文件夹的整个挪进回收站，回应同删
//! 一篇；`paths` 几样一起删，回应照先后；有一样不在就一样都不删；重了的、一样在另一样里面的、两样都写或都没写的
//! `bad_params`。
//!
//! 回收站：有的机器上临时目录那块盘没有能放的，照实报 `trash_failed`（同 `knowledge_manage.rs`），这时只看拒绝的那几条。

use serde_json::{Value, json};

use crate::knowledge_manage::{client, upload};
use crate::support::*;

/// 建库 `own`，放四篇：`cats/a.md`、`cats/b.md`、`dogs/c.md`、`top.md`。
async fn filled(client: &mut Client) {
    client.call("c", "kb.create", json!({"kb": "own"})).await;
    for (n, path) in ["cats/a.md", "cats/b.md", "dogs/c.md", "top.md"]
        .into_iter()
        .enumerate()
    {
        let name = path.rsplit('/').next().unwrap_or(path);
        let blob = upload(client, &format!("b{n}"), name, "猫\n").await;
        let reply = client
            .call(
                &format!("a{n}"),
                "kb.add",
                json!({"kb": "own", "path": path, "blob": blob}),
            )
            .await;
        assert_eq!(reply["result"], json!({}), "{reply}");
    }
}

/// 库 `own` 里现在有几篇。
async fn documents(client: &mut Client) -> Value {
    let rebuilt = client.call("r", "kb.reindex", json!({"kb": "own"})).await;
    rebuilt["result"]["documents"].clone()
}

#[tokio::test]
async fn a_folder_and_several_paths_go_to_the_trash() {
    let home = Home::new();
    let mut client = client(&home).await;
    filled(&mut client).await;
    let folder = client
        .call("d1", "kb.remove", json!({"kb": "own", "path": "cats"}))
        .await;
    if reason(&folder) == Some("trash_failed") {
        return;
    }
    assert!(folder["result"]["trash"].is_string(), "{folder}");
    assert_eq!(documents(&mut client).await, json!(2), "整个文件夹没了");
    let several = client
        .call(
            "d2",
            "kb.remove",
            json!({"kb": "own", "paths": ["top.md", "dogs"]}),
        )
        .await;
    let trash = several["result"]["trash"]
        .as_array()
        .unwrap_or_else(|| panic!("{several}"));
    assert_eq!(trash.len(), 2, "{several}");
    assert_eq!(documents(&mut client).await, json!(0));
}

#[tokio::test]
async fn wrong_removals_remove_nothing() {
    let home = Home::new();
    let mut client = client(&home).await;
    filled(&mut client).await;
    for (params, want) in [
        (
            json!({"kb": "own", "paths": ["top.md", "gone"]}),
            "unknown_document",
        ),
        (
            json!({"kb": "own", "paths": ["cats", "cats/a.md"]}),
            "bad_params",
        ),
        (
            json!({"kb": "own", "paths": ["top.md", "top.md"]}),
            "bad_params",
        ),
        (json!({"kb": "own", "paths": []}), "bad_params"),
        (
            json!({"kb": "own", "path": "top.md", "paths": ["dogs"]}),
            "bad_params",
        ),
        (json!({"kb": "own"}), "bad_params"),
        (json!({"kb": "own", "paths": ["../x"]}), "bad_params"),
    ] {
        let reply = client.call("d", "kb.remove", params.clone()).await;
        assert_eq!(reason(&reply), Some(want), "{params}：{reply}");
    }
    assert_eq!(documents(&mut client).await, json!(4), "一样都没删");
}

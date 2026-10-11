//! 知识库的协议（施工 K-1 上，`docs/blueprint/knowledge.md`「对外的样子」，`protocol.md` 的 `kb.*`）：真核心，知识库是数据根
//! 里的真目录。`kb.list`、`kb.search`、`kb.read` 的回应一格一格对；写错的几种 `bad_params`，没有的库 `unknown_kb`，没有的
//! 文档 `unknown_document`。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

fn shelf(home: &Home) {
    let dir = home.root.account_dir(&alice()).join("kb");
    let notes = dir.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(
        notes.join("kb.toml"),
        "name = \"日语笔记\"\ndescription = \"学日语的笔记\"\n",
    )
    .expect("写得进");
    std::fs::write(
        notes.join("grammar.md"),
        "# 语法\n\n## て形\n\n食べる → 食べて\n",
    )
    .expect("写得进");
    let broken = dir.join("broken");
    std::fs::create_dir_all(&broken).expect("建得了");
    std::fs::write(broken.join("kb.toml"), "title = \"错的格\"\n").expect("写得进");
}

async fn client(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

#[tokio::test]
async fn list_search_and_read() {
    let home = Home::new();
    shelf(&home);
    let mut client = client(&home).await;
    let listed = client.call("l1", "kb.list", json!({})).await;
    let bases = &listed["result"]["bases"];
    assert_eq!(bases[0]["id"], "broken", "{listed}");
    assert!(
        bases[0]["error"].is_string(),
        "kb.toml 写错的带为什么：{listed}"
    );
    assert_eq!(
        bases[1],
        json!({"id": "notes", "name": "日语笔记", "description": "学日语的笔记", "documents": 1, "chunks": 1}),
        "{listed}"
    );

    let found = client
        .call("s1", "kb.search", json!({"query": "食べて"}))
        .await;
    let hit = &found["result"]["hits"][0];
    assert_eq!(hit["kb"], "notes", "{found}");
    assert_eq!(hit["path"], "grammar.md");
    assert_eq!(hit["chunk"], 0);
    assert_eq!(hit["heading"], "语法 > て形");
    assert_eq!(hit["text"], "食べる → 食べて");
    assert!(
        hit["at"].as_str().is_some_and(|at| at.ends_with('Z')),
        "{hit}"
    );
    let only = client
        .call(
            "s2",
            "kb.search",
            json!({"query": "食べて", "kb": "broken"}),
        )
        .await;
    assert_eq!(only["result"], json!({"hits": []}), "只搜那一个：{only}");

    let read = client
        .call(
            "r1",
            "kb.read",
            json!({"kb": "notes", "path": "grammar.md"}),
        )
        .await;
    assert_eq!(
        read["result"],
        json!({"text": "# 语法\n\n## て形\n\n食べる → 食べて\n", "page": 1, "pages": 1}),
        "{read}"
    );
}

#[tokio::test]
async fn wrong_requests_are_refused() {
    let home = Home::new();
    shelf(&home);
    let mut client = client(&home).await;
    let cases: [(&str, Value, &str); 11] = [
        ("kb.nope", json!({}), "unknown_method"),
        (
            "kb.read",
            json!({"kb": "Bad Name", "path": "a.md"}),
            "bad_params",
        ),
        ("kb.search", json!({"query": "  "}), "bad_params"),
        ("kb.search", json!({}), "bad_params"),
        (
            "kb.search",
            json!({"query": "猫", "limit": 0}),
            "bad_params",
        ),
        (
            "kb.search",
            json!({"query": "猫", "limit": 51}),
            "bad_params",
        ),
        (
            "kb.search",
            json!({"query": "猫", "kb": "Bad Name"}),
            "bad_params",
        ),
        (
            "kb.search",
            json!({"query": "猫", "kb": "nope"}),
            "unknown_kb",
        ),
        (
            "kb.read",
            json!({"kb": "notes", "path": "../grammar.md"}),
            "unknown_document",
        ),
        (
            "kb.read",
            json!({"kb": "notes", "path": "grammar.md", "page": 0}),
            "bad_params",
        ),
        (
            "kb.read",
            json!({"kb": "notes", "path": "grammar.md", "page": 2}),
            "bad_params",
        ),
    ];
    for (n, (method, params, why)) in cases.into_iter().enumerate() {
        let reply = client.call(&format!("x{n}"), method, params.clone()).await;
        assert_eq!(reason(&reply), Some(why), "{method} {params}：{reply}");
    }
    let reply = client
        .call("y", "kb.read", json!({"kb": "nope", "path": "a.md"}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_kb"), "{reply}");
}

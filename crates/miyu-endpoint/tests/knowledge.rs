//! 知识库的协议（施工 K-1 上，`docs/blueprint/knowledge.md`「对外的样子」，`protocol.md` 的 `kb.*`）：真核心，知识库是数据根
//! 里的真目录。`kb.list`、`kb.search`、`kb.read` 的回应一格一格对；写错的几种 `bad_params`，没有的库 `unknown_kb`，没有的
//! 文档 `unknown_document`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::embedding;
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
    // 施工 W-15：建好了的带上次建好的时刻（这一刻的，另查），这时没在建。
    let mut notes = bases[1].clone();
    let indexed_at = notes
        .as_object_mut()
        .and_then(|notes| notes.remove("indexed_at"));
    assert!(
        indexed_at
            .as_ref()
            .and_then(|at| at.as_str())
            .is_some_and(|at| at.ends_with('Z')),
        "{listed}"
    );
    assert_eq!(
        notes,
        json!({"id": "notes", "name": "日语笔记", "description": "学日语的笔记", "documents": 1, "chunks": 1, "auto": false, "indexing": false}),
        "{listed}"
    );

    let found = client
        .call("s1", "kb.search", json!({"query": "食べて"}))
        .await;
    let hit = &found["result"]["hits"][0];
    assert_eq!(hit["kb"], "notes", "{found}");
    assert_eq!(hit["path"], "grammar.md");
    assert_eq!(hit["chunk"], 0);
    assert_eq!(hit["heading"], "语法 › て形");
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

#[tokio::test]
async fn kb_search_finds_by_meaning_once_the_vectors_are_filled() {
    // 施工 K-2：问句照这时的配置算向量（远程的假服务器，什么都挺像），搜完在后台补搜过的库缺的向量；补上以后关键词对不上的
    // 也找得到。
    let server = miyu_http::testkit::Server::start(vec![embedding::vector(); 40]).await;
    let home = Home::new();
    let dir = home.root.account_dir(&alice()).join("kb").join("notes");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("cat.md"), "团子的饭是鸡胸肉\n").expect("写得进");
    let mut client = Client::connect(embedding::remote(&home, &server));
    client.hello().await;
    let mut hits = Value::Null;
    for n in 0..600 {
        let reply = client
            .call(&format!("s{n}"), "kb.search", json!({"query": "猫吃什么"}))
            .await;
        hits = reply["result"]["hits"].clone();
        if hits.as_array().is_some_and(|hits| !hits.is_empty()) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(hits[0]["text"], "团子的饭是鸡胸肉", "{hits}");
    assert!(
        server
            .received()
            .iter()
            .any(|request| request.path == "/v1/embeddings")
    );
}

/// 她的 `kb_search` 照 `query` 搜一次，接着说一句。
fn searching(query: &str) -> [Play; 2] {
    let args = json!({ "query": query }).to_string();
    [Play::calls(&[("kb_search", &args)]), Play::Says("嗯。")]
}

/// 库 `notes` 的块都补上了远程的向量没有。
fn filled(home: &Home) -> bool {
    let knowledge = miyu_store::knowledge::Knowledge::new(&home.root);
    let index = knowledge.index_of(&alice(), "notes");
    index
        .missing("emb/text-emb", 0, 10)
        .is_ok_and(|missing| missing.is_empty())
        && index.keys().is_ok_and(|keys| !keys.is_empty())
}

#[tokio::test]
async fn her_kb_search_finds_by_meaning_and_still_does_after_a_restart() {
    // 施工 K-2：核心把算向量的交给造的会话（搜完补上向量）和载入的会话（问句照意思找）。
    let server = miyu_http::testkit::Server::start(vec![embedding::vector(); 40]).await;
    let home = Home::new();
    let dir = home.root.account_dir(&alice()).join("kb").join("notes");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("cat.md"), "团子的饭是鸡胸肉\n").expect("写得进");
    let script = Script::new([searching("猫吃什么"), searching("猫吃什么")].concat());
    let tools = || {
        let tools = miyu_knowledge::tools(&default_resources()).expect("读得出");
        Catalog::in_packages([(miyu_knowledge::PACKAGE, tools)]).expect("合写法")
    };
    let core = embedding::remote_with(&home, &server, &script, tools());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "猫吃什么").await;
    home.until_turns(&session, 1).await;
    for _ in 0..600 {
        if filled(&home) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(filled(&home), "造的会话搜完补上了");
    drop(client);
    drop(core);
    let again = embedding::remote_with(&home, &server, &script, tools());
    let mut client = Client::connect(again);
    client.hello().await;
    client.say("s2", &session, "再找找").await;
    home.until_turns(&session, 2).await;
    let found = home
        .log(&session)
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        });
    assert!(
        found
            .as_deref()
            .is_some_and(|found| found.contains("团子的饭是鸡胸肉")),
        "载入的会话照意思找得到：{found:?}"
    );
}

#[tokio::test]
async fn a_mounted_folder_follows_the_home_and_an_unplugged_one_is_skipped() {
    // 施工 K-4：`~/` 照核心的家目录；挂的目录用不了的 `kb.list` 带 `error`、数照索引里的，`kb.search` 跳过它、不算出错。
    let home = Home::new();
    let notes = home.work.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("cat.md"), "团子是橘猫\n").expect("写得进");
    let dir = home.root.account_dir(&alice()).join("kb").join("notes");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("kb.toml"), "mount = \"~/notes\"\n").expect("写得进");
    let core = home.core_full(
        &Script::new([]),
        Catalog::default(),
        Some(home.work.clone()),
        TOKEN,
    );
    let mut client = Client::connect(Arc::new(core));
    client.hello().await;
    let search = json!({"query": "橘猫"});
    let found = client.call("s1", "kb.search", search.clone()).await;
    assert_eq!(found["result"]["hits"][0]["path"], "cat.md", "{found}");
    std::fs::rename(&notes, home.work.join("unplugged")).expect("挪得走");
    let listed = client.call("l1", "kb.list", json!({})).await;
    let base = &listed["result"]["bases"][0];
    assert!(base["error"].is_string(), "{listed}");
    assert_eq!(
        (&base["documents"], &base["chunks"]),
        (&json!(1), &json!(1))
    );
    let skipped = client.call("s2", "kb.search", search).await;
    assert_eq!(skipped["result"], json!({"hits": []}), "{skipped}");
}

#[tokio::test]
async fn a_pdf_hit_says_its_page_and_reads_by_page() {
    // 施工 K-5：PDF 的那一块多一格 `page`，别的不写；`kb.read` 读 PDF 是它的第几页，`pages` 是 PDF 的页数。
    let home = Home::new();
    shelf(&home);
    let dir = home.root.account_dir(&alice()).join("kb").join("notes");
    let pdf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../miyu-extract/tests/fixtures/cat.pdf");
    std::fs::copy(pdf, dir.join("cat.pdf")).expect("拷得了");
    let core = home
        .core_full(&Script::new([]), Catalog::default(), None, TOKEN)
        .with_extractor(Some(extractor()));
    let mut client = Client::connect(Arc::new(core));
    client.hello().await;
    let found = client
        .call("s1", "kb.search", json!({"query": "October"}))
        .await;
    let hit = &found["result"]["hits"][0];
    assert_eq!(
        (&hit["path"], &hit["page"]),
        (&json!("cat.pdf"), &json!(2)),
        "{found}"
    );
    let plain = client
        .call("s2", "kb.search", json!({"query": "食べて"}))
        .await;
    assert!(plain["result"]["hits"][0].get("page").is_none(), "{plain}");
    let read = client
        .call(
            "r1",
            "kb.read",
            json!({"kb": "notes", "path": "cat.pdf", "page": 3}),
        )
        .await;
    assert_eq!(
        read["result"],
        json!({"text": "第三页：体重 4.2 公斤。", "page": 3, "pages": 3}),
        "{read}"
    );
}

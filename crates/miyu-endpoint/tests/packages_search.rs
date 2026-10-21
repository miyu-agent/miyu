//! 在软件源里搜（施工 G-3 下，设计 `37-软件源.md` 第四节第 3 条）：真核心走一遍，假服务器当软件源。照编号、名字、说明搜，
//! 几个词都得有、不分大小写，空的列全部；名字照连接的语言挑；带装着的版本、官方、拉黑的理由；搜几次只拿一次索引。

use serde_json::{Value, json};

use miyu_http::testkit::Server;
use miyu_source::sign;

use crate::support::source::{core, entry, keys, serving};
use crate::support::*;

async fn search(client: &mut Client, query: &str) -> Value {
    client
        .call("q", "package.search", json!({"query": query}))
        .await
}

fn ids(reply: &Value) -> Vec<String> {
    reply["result"]["packages"]
        .as_array()
        .expect("有")
        .iter()
        .map(|row| row["package"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn the_index_is_searched_by_id_name_and_summary() {
    let home = Home::new();
    let (secret, trusted) = keys();
    // 给 im 带了内容的（施工 G-9）：索引的 `provides` 里有 `content`。
    let mut memes = entry("memes", json!({"en": "Memes", "zh": "表情包"}), json!({}));
    memes["provides"] = json!({"content": ["im"], "skills": ["sticker"]});
    memes["depends"] = json!(["im"]);
    // 包的种类（施工 G-11）：新生成的索引带着。
    memes["kind"] = json!("bundle");
    let index = serde_json::to_vec(&json!({
        "version": 1,
        "generated": "2026-10-19T03:00:00Z",
        "packages": [
            entry("embed", json!({"en": "Embedding model", "zh": "语义模型"}), json!({"en": "Local embeddings"})),
            memes,
            entry("evil", json!({"en": "Evil"}), json!({})),
        ],
        "blocked": [{"id": "evil", "reason": "malware"}],
    }))
    .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, &secret).expect("签得了")).expect("写得成");
    let server = Server::start(serving(&(index, signature))).await;
    // 本机装着一份 memes（从文件夹装的，版本 0.9）。
    let folder = home.work.join("memes");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nversion = \"0.9\"\nname = { en = \"Memes\" }\n\n[content.im]\ndir = \"stickers\"\n",
    )
    .expect("写得进");
    std::fs::create_dir_all(folder.join("stickers")).expect("建得了");
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;
    let installed = client
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "memes", "{installed}");

    let all = search(&mut client, "").await;
    assert_eq!(ids(&all), ["embed", "memes", "evil"], "{all}");
    assert_eq!(all["result"]["generated"], "2026-10-19T03:00:00Z");
    let rows = all["result"]["packages"].as_array().expect("有");
    assert_eq!(rows[0]["name"], "语义模型", "照连接的语言挑：{all}");
    assert_eq!(
        rows[0]["summary"], "Local embeddings",
        "没有这种语言的照英文"
    );
    assert_eq!(rows[0]["installed"], Value::Null);
    assert_eq!(rows[0]["official"], true);
    assert_eq!(rows[0]["version"], "1.0");
    assert_eq!(rows[1]["installed"], "0.9", "装着的版本");
    assert_eq!(rows[2]["blocked"], "malware");
    assert_eq!(rows[0]["blocked"], Value::Null);
    assert_eq!(rows[1]["contents"], json!(["im"]), "给 im 带了内容的");
    assert_eq!(rows[0]["contents"], json!([]), "没带的是空的");
    // 带了哪几样、依赖哪几个包（施工 G-10）：索引里的原样，没有的是空的。
    assert_eq!(
        rows[1]["provides"],
        json!({"content": ["im"], "skills": ["sticker"]})
    );
    assert_eq!(rows[1]["depends"], json!(["im"]));
    assert_eq!(rows[2]["provides"], json!({}));
    assert_eq!(rows[2]["depends"], json!([]));
    assert_eq!(rows[1]["kind"], "bundle", "包的种类照索引（施工 G-11）");
    assert!(
        rows[0].get("kind").is_none(),
        "以前生成的索引没有的不写：{all}"
    );
    // 装着的那一份照它自己的清单（施工 G-9）。
    let listed = client.call("l", "package.list", json!({})).await;
    let packages = listed["result"]["packages"].as_array().expect("有");
    let memes = packages
        .iter()
        .find(|one| one["package"] == "memes")
        .expect("装着");
    assert_eq!(memes["contents"], json!(["im"]), "{memes}");
    assert!(
        packages
            .iter()
            .filter(|one| one["package"] != "memes" && one.get("code").is_none())
            .all(|one| one.get("contents").is_none()),
        "别的没带：{listed}"
    );

    assert_eq!(ids(&search(&mut client, "语义").await), ["embed"]);
    assert_eq!(
        ids(&search(&mut client, "EMBEDDING local").await),
        ["embed"],
        "不分大小写、几个词都得有"
    );
    assert!(ids(&search(&mut client, "embedding memes").await).is_empty());
    assert_eq!(
        ids(&search(&mut client, "表情").await),
        ["memes"],
        "别的语言的名字也搜得到"
    );
    assert_eq!(server.received().len(), 2, "搜几次只拿一次索引");
}

#[tokio::test]
async fn searching_without_a_source_is_refused() {
    let home = Home::new();
    let (_, trusted) = keys();
    let (other, _) = keys();
    let server = Server::start(serving(&crate::support::source::signed(
        "2026-10-19T03:00:00Z",
        &["x"],
        &other,
    )))
    .await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;
    let got = search(&mut client, "x").await;
    assert_eq!(reason(&got), Some("source_unreachable"), "{got}");
}

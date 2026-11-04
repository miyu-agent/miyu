//! 把别处的记忆导进来（施工 MI-1，`docs/blueprint/memory.md`「协议」的 `memory.import`）：真核心。整批照先后记下，一条的
//! 时刻是原来的、`by` 是人、出处空；一条不对整批不写，说出第几条、哪一项；同一个 `origin` 不再导，作废、清掉了的也算；导完
//! 在后台给没带关键词的写关键词，从没整理过的这一间接着整理一次，刚整理过的不起。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_http::testkit::Server;
use miyu_session::testkit::Script;

use crate::support::memories::connected;
use crate::support::providers::{data, profiles, routed, said};
use crate::support::*;

/// 导一批，交回回应。
async fn import(client: &mut Client, id: &str, items: Value) -> Value {
    client
        .call(
            id,
            "memory.import",
            json!({"persona": "engineer", "items": items}),
        )
        .await
}

/// 列出这一间的全部（作废的也要），旧的在前。
async fn listed(client: &mut Client, id: &str) -> Vec<Value> {
    let reply = client
        .call(
            id,
            "memory.list",
            json!({"persona": "engineer", "forgotten": true}),
        )
        .await;
    let mut all = reply["result"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("有 memories：{reply}"))
        .clone();
    all.reverse();
    all
}

fn one(at: &str, class: &str, text: &str, origin: &str) -> Value {
    json!({"at": at, "class": class, "text": text, "origin": origin})
}

#[tokio::test]
async fn imported_memories_keep_their_time_in_order() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let reply = import(
        &mut client,
        "i1",
        json!([
            {"at": "2026-10-21T10:53:22.120456+08:00", "class": "user", "text": " 用户的网络是光猫接主路由 ",
             "origin": "legacy:default:facts:7", "keywords": "网络, 路由器"},
            {"at": "2026-10-23T12:45:06.820Z", "class": "person", "text": "小林在组装 NAS",
             "origin": "legacy:default:facts:9", "who": " qq:10086 "},
            {"at": "2026-09-05T04:22:35Z", "class": "episode", "text": "2026-09-05 和小林聊了显卡",
             "origin": "legacy:dev:facts:2", "people": ["qq:10086"]},
        ]),
    )
    .await;
    assert_eq!(
        reply["result"],
        json!({"ids": ["m1", "m2", "m3"], "imported": 3, "skipped": 0}),
        "{reply}"
    );
    let all = listed(&mut client, "l1").await;
    let shape: Vec<(Value, Value, Value)> = all
        .iter()
        .map(|memory| {
            (
                memory["id"].clone(),
                memory["at"].clone(),
                memory["text"].clone(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            (
                json!("m1"),
                json!("2026-10-21T02:53:22.120Z"),
                json!("用户的网络是光猫接主路由")
            ),
            (
                json!("m2"),
                json!("2026-10-23T12:45:06.820Z"),
                json!("小林在组装 NAS")
            ),
            (
                json!("m3"),
                json!("2026-09-05T04:22:35.000Z"),
                json!("2026-09-05 和小林聊了显卡")
            ),
        ],
        "编号照先后，时刻是原来的（换成 UTC），正文去掉前后空白"
    );
    for memory in &all {
        assert_eq!(memory["by"], json!("person"), "人记的：{memory}");
        assert_eq!(memory["sources"], json!([]), "出处空：{memory}");
    }
    assert_eq!(all[0]["keywords"], json!("网络, 路由器"));
    assert_eq!(all[1]["who"], json!("qq:10086"), "是谁去掉前后空白");
    assert_eq!(all[2]["people"], json!(["qq:10086"]));
    let found = client
        .call(
            "s1",
            "memory.search",
            json!({"persona": "engineer", "query": "路由器"}),
        )
        .await;
    assert_eq!(
        found["result"]["memories"][0]["id"],
        json!("m1"),
        "照关键词搜得到：{found}"
    );
}

#[tokio::test]
async fn one_wrong_item_writes_none_and_says_which() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let at = "2026-10-21T02:53:22.120Z";
    let good = one(at, "user", "用户养猫", "o:1");
    let with = |field: &str, value: Value| {
        let mut item = good.clone();
        item[field] = value;
        item
    };
    let long = "字".repeat(121);
    let cases: Vec<(Value, Value)> = vec![
        (json!([]), json!({"field": "items"})),
        (json!(vec![good.clone(); 501]), json!({"field": "items"})),
        (
            json!([good.clone(), "不是一条"]),
            json!({"index": 1, "field": "item"}),
        ),
        (
            json!([with("at", Value::Null)]),
            json!({"index": 0, "field": "at"}),
        ),
        (
            json!([with("at", json!("昨天"))]),
            json!({"index": 0, "field": "at"}),
        ),
        (
            json!([with("at", json!("2026-10-21T02:53:22"))]),
            json!({"index": 0, "field": "at"}),
        ),
        (
            json!([with("at", json!("2999-01-01T00:00:00Z"))]),
            json!({"index": 0, "field": "at"}),
        ),
        (
            json!([with("class", json!("fact"))]),
            json!({"index": 0, "field": "class"}),
        ),
        (
            json!([with("text", json!("  "))]),
            json!({"index": 0, "field": "text"}),
        ),
        (
            json!([with("text", json!(long))]),
            json!({"index": 0, "field": "text"}),
        ),
        (
            json!([with("class", json!("person"))]),
            json!({"index": 0, "field": "who"}),
        ),
        (
            json!([with("who", json!("qq:1"))]),
            json!({"index": 0, "field": "who"}),
        ),
        (
            json!([with("people", json!(["qq:1"]))]),
            json!({"index": 0, "field": "people"}),
        ),
        (
            json!([with("keywords", json!("词".repeat(201)))]),
            json!({"index": 0, "field": "keywords"}),
        ),
        (
            json!([with("origin", json!(" "))]),
            json!({"index": 0, "field": "origin"}),
        ),
        (
            json!([good.clone(), good.clone()]),
            json!({"index": 1, "field": "origin"}),
        ),
    ];
    for (n, (items, expected)) in cases.into_iter().enumerate() {
        let reply = import(&mut client, &format!("i{n}"), items).await;
        assert_eq!(reason(&reply), Some("bad_params"), "第 {n} 种：{reply}");
        let mut data = reply["error"]["data"].clone();
        data.as_object_mut().expect("有 data").remove("reason");
        assert_eq!(data, expected, "第 {n} 种");
    }
    assert!(listed(&mut client, "l1").await.is_empty(), "一条都没记");
    let reply = import(
        &mut client,
        "ok",
        json!([with("text", json!("字".repeat(120)))]),
    )
    .await;
    assert_eq!(
        reply["result"]["imported"],
        json!(1),
        "120 字记得下：{reply}"
    );
}

#[tokio::test]
async fn the_same_origin_is_never_imported_twice() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let at = "2026-10-21T02:53:22.120Z";
    let first = json!([
        one(at, "user", "用户养猫", "o:1"),
        one(at, "user", "用户住北京", "o:2")
    ]);
    let reply = import(&mut client, "i1", first.clone()).await;
    assert_eq!(reply["result"]["imported"], json!(2), "{reply}");
    let again = import(&mut client, "i1", first).await;
    assert_eq!(
        again["result"],
        json!({"ids": [null, null], "imported": 0, "skipped": 2}),
        "同一个命令编号再发：照 origin 全跳过 {again}"
    );
    let reply = import(
        &mut client,
        "i2",
        json!([
            one(at, "user", "用户养了两只猫", "o:1"),
            one(at, "user", "用户喝咖啡", "o:3")
        ]),
    )
    .await;
    assert_eq!(
        reply["result"],
        json!({"ids": [null, "m3"], "imported": 1, "skipped": 1}),
        "{reply}"
    );
    let forgot = client
        .call(
            "f1",
            "memory.forget",
            json!({"persona": "engineer", "id": "m3"}),
        )
        .await;
    assert_eq!(forgot["result"], json!({}), "{forgot}");
    let cleared = client
        .call(
            "f2",
            "memory.forget",
            json!({"persona": "engineer", "clear": "me"}),
        )
        .await;
    assert!(cleared["result"]["cleared"].is_number(), "{cleared}");
    let reply = import(
        &mut client,
        "i3",
        json!([
            one(at, "user", "用户喝咖啡", "o:3"),
            one(at, "user", "用户住北京", "o:2")
        ]),
    )
    .await;
    assert_eq!(
        reply["result"]["skipped"],
        json!(2),
        "作废了的、清掉了的也不再导：{reply}"
    );
}

/// 一家 `a` 在假服务器上，`models.chat` 是 `a/m`（整理记忆、写关键词照它）。
async fn routed_client(home: &Home, server: &Server) -> Client {
    home.write(
        "etc/config.toml",
        &format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n",
            server.base_url
        ),
    );
    let mut client = Client::connect(routed(home, &[], data(profiles(json!({})))));
    client.hello().await;
    client
}

/// 假服务器收到 `count` 个请求（最多等 10 秒），再等一会儿看没有多的。
async fn requests_settle_at(server: &Server, count: usize) {
    for _ in 0..200 {
        if server.received().len() >= count {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(server.received().len(), count, "收到的请求数");
}

/// 第 `at` 个请求的最后一条 user 的字。
fn asked(server: &Server, at: usize) -> String {
    let body: Value = serde_json::from_slice(&server.received()[at].body).expect("请求体是 JSON");
    body["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .and_then(|message| message["content"].as_str().map(str::to_string))
        .unwrap_or_else(|| panic!("一条 user 的字：{body}"))
}

#[tokio::test]
async fn after_an_import_keywords_are_written_and_a_first_merge_runs() {
    let server = Server::start(vec![
        said(&json!({"keywords": [{"id": "m1", "keywords": "宠物,猫咪"}, {"id": "m2", "keywords": "城市"}]}).to_string()),
        said(&json!({"summary": "用户养猫，住北京。"}).to_string()),
        said(&json!({"keywords": []}).to_string()),
    ])
    .await;
    let home = Home::new();
    let mut client = routed_client(&home, &server).await;
    let at = "2026-10-21T02:53:22.120Z";
    let reply = import(
        &mut client,
        "i1",
        json!([
            one(at, "user", "用户养了一只猫", "o:1"),
            one(at, "user", "用户住北京", "o:2")
        ]),
    )
    .await;
    assert_eq!(reply["result"]["imported"], json!(2), "{reply}");
    // 没带关键词的另发一次写上，这一间从没整理过：接着整理一次（施工 E-9）。
    requests_settle_at(&server, 2).await;
    let tagging = asked(&server, 0);
    assert!(
        tagging.starts_with("Below are memories you keep."),
        "{tagging}"
    );
    assert!(
        tagging.contains("用户养了一只猫") && tagging.contains("用户住北京"),
        "{tagging}"
    );
    assert!(asked(&server, 1).contains("用户住北京"), "整理交进去新记的");
    let all = listed(&mut client, "l1").await;
    assert_eq!(all[0]["keywords"], json!("宠物,猫咪"), "{all:?}");
    // 带了关键词的不发；刚整理过、不满间隔的不整理。
    let reply = import(
        &mut client,
        "i2",
        json!([{"at": at, "class": "user", "text": "用户喝茶", "origin": "o:3", "keywords": "茶"}]),
    )
    .await;
    assert_eq!(reply["result"]["imported"], json!(1), "{reply}");
    requests_settle_at(&server, 2).await;
    let reply = import(
        &mut client,
        "i3",
        json!([one(at, "user", "用户喝咖啡", "o:4")]),
    )
    .await;
    assert_eq!(reply["result"]["imported"], json!(1), "{reply}");
    requests_settle_at(&server, 3).await;
    assert!(
        asked(&server, 2).contains("用户喝咖啡"),
        "只给这一条写关键词"
    );
}

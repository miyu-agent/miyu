//! 用途给人看的名字（施工 UP-3，`docs/blueprint/models.md`「协议」`usage.query`）：照 `purpose` 分组的每一行带 `purpose_name`，
//! 照连接的语言：核心自己的照字表（没写用途的主请求是「主对话」），扩展写的照包清单 `[[purpose]]` 声明的（几个包声明同一个的
//! 照包的编号排第一的，核心自己有的扩展改不了名），都没有的不写。不照 `purpose` 分组的不带。

use serde_json::{Value, json};

use crate::support::providers::{data, profiles, routed_serving, said};
use crate::support::*;
use miyu_http::testkit::Server;

/// 一个不拉起的扩展包 `id`，清单里再接上 `rest`。
fn install(home: &Home, id: &str, rest: &str) {
    home.write(
        &format!("opt/{id}/package.toml"),
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"P\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{id}\"\nabout = {{ en = \"P\" }}\n\n[process]\nstart = \"manual\"\n\n{rest}"
        ),
    );
}

/// 用 `locale` 握手的连接。
async fn connected(core: &std::sync::Arc<miyu_endpoint::Core>, locale: &str) -> Client {
    let mut client = Client::connect(std::sync::Arc::clone(core));
    let hello = client
        .call(
            "hello-1",
            "hello",
            json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
                   "locale": locale, "caps": {"input": true}, "token": TOKEN}),
        )
        .await;
    assert!(hello.get("error").is_none(), "{hello}");
    client
}

/// 照 `group` 查，交回每一行的用途和名字（没有名字的是 `null`），照先后。
async fn named(client: &mut Client, group: Value) -> Vec<(Value, Value)> {
    let reply = client
        .call("q", "usage.query", json!({"group": group}))
        .await;
    reply["result"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|row| (row["purpose"].clone(), row["purpose_name"].clone()))
        .collect()
}

#[tokio::test]
async fn rows_by_purpose_carry_a_name_in_the_connection_s_language() {
    let server = Server::start(vec![said("a"), said("b"), said("c"), said("d"), said("e")]).await;
    let home = Home::new();
    home.write(
        "etc/config.toml",
        &format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nlocal = false\n\n[models]\nchat = \"a/m\"\n",
            server.base_url
        ),
    );
    let join =
        "[[purpose]]\nid = \"join-approval\"\nname = { zh = \"入群审核\", en = \"Join review\" }\n";
    install(
        &home,
        "im",
        &format!("{join}\n[[purpose]]\nid = \"title\"\nname = {{ zh = \"冒名\" }}\n"),
    );
    install(&home, "zz", &join.replace("入群审核", "排在后面的"));
    let core = routed_serving(&home, data(profiles(json!({}))));
    let mut client = connected(&core, "zh-CN").await;
    for (id, purpose) in [
        ("c1", "join-approval"),
        ("c2", "title"),
        ("c3", "vision"),
        ("c4", "plain"),
    ] {
        let asked = json!({"model": "a/m", "purpose": purpose, "messages": [{"role": "user", "text": "hi"}]});
        let reply = client.call(id, "model.call", asked).await;
        assert!(reply.get("result").is_some(), "{reply}");
    }
    let made = client
        .call("s1", "session.create", json!({"cwd": "~", "titles": false}))
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    client.say("s2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    assert_eq!(
        named(&mut client, json!(["purpose"])).await,
        [
            (Value::Null, json!("主对话")),
            (json!("join-approval"), json!("入群审核")),
            (json!("plain"), Value::Null),
            (json!("title"), json!("起标题")),
            (json!("vision"), json!("看图")),
        ],
        "没写用途的是主对话；扩展声明的照编号排第一的包；核心自己的扩展改不了名；都没有的不写"
    );
    let mut english = connected(&core, "en-US").await;
    let names: Vec<Value> = named(&mut english, json!(["purpose"]))
        .await
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    assert_eq!(
        names,
        [
            json!("Conversation"),
            json!("Join review"),
            Value::Null,
            json!("Titles"),
            json!("Vision")
        ]
    );
    let reply = client
        .call("q2", "usage.query", json!({"group": ["model"]}))
        .await;
    assert!(
        reply["result"]["rows"][0].get("purpose_name").is_none(),
        "不照用途分组的不带：{reply}"
    );
}

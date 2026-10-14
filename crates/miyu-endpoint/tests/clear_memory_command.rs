//! 斜杠命令 `/clear-memory`（别名 `/reset-memory`，施工 R-3 五补，`docs/blueprint/protocol.md`「`command.run`」第 12 条）：真
//! 核心、真记忆日志。清掉这个会话产生的记忆（出处全在这个会话里的），别的会话来的、人记的不动；不先问，回执说清了几条，一条
//! 都没有的说没有；记一条 `command.ran`，不请求模型。记忆没开的回 `memory_unavailable`。

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::memories::{pairs, texts, with_persona};
use crate::support::*;

async fn run(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(id, "command.run", json!({"session": session, "text": text}))
        .await
}

fn noted(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::CommandRan(ran) => Some(ran.command.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn clear_memory_takes_only_what_this_session_made() {
    let home = Home::new();
    let remember = |text: &str| {
        Play::calls(&[(
            "remember",
            &json!({"class": "user", "text": text}).to_string(),
        )])
    };
    let script = Script::new([
        remember("用户养了一只猫"),
        Play::Says("记住了。"),
        remember("用户爱喝茶"),
        Play::Says("记住了。"),
    ]);
    let resources = default_resources();
    let mut tools = miyu_basesystem::tools(&resources).expect("读得出");
    tools.extend(miyu_memory::tools(&resources).expect("读得出"));
    let catalog = Catalog::new(tools).expect("合写法");
    let mut client = Client::connect(with_persona(&home, &script, catalog));
    client.hello().await;
    let cat = client.create("c1", "~").await;
    client.say("c2", &cat, "我养了一只猫").await;
    home.until_turns(&cat, 1).await;
    let tea = client.create("c3", "~").await;
    client.say("c4", &tea, "我爱喝茶").await;
    home.until_turns(&tea, 1).await;
    let kept = run(&mut client, "k1", &cat, "/remember 用户住在上海").await;
    assert_eq!(kept["result"]["command"], "remember", "{kept}");
    let asked = script.requests().len();

    let reply = run(&mut client, "k2", &cat, "/clear-memory").await;
    assert_eq!(reply["result"]["command"], "clear-memory", "{reply}");
    assert_eq!(reply["result"]["said"], "清掉了这个会话记下的 1 条记忆。");
    let listed = client
        .call("l1", "memory.list", json!({"forgotten": true}))
        .await;
    assert_eq!(
        texts(&listed),
        pairs(&[("m3", "用户住在上海"), ("m2", "用户爱喝茶")]),
        "别的会话来的、人记的不动；清掉的作废的里也没有"
    );
    let again = run(&mut client, "k3", &cat, "/clear-memory").await;
    assert_eq!(again["result"]["said"], "这个会话没有记下的记忆。");
    let same = run(&mut client, "k2", &cat, "/clear-memory").await;
    assert_eq!(same["result"], reply["result"], "同一个编号只算一次");

    let alias = run(&mut client, "k4", &tea, "/reset-memory 跟了字也不看").await;
    assert_eq!(alias["result"]["command"], "clear-memory", "{alias}");
    assert_eq!(alias["result"]["said"], "清掉了这个会话记下的 1 条记忆。");
    let listed = client.call("l2", "memory.list", json!({})).await;
    assert_eq!(texts(&listed), pairs(&[("m3", "用户住在上海")]));
    assert_eq!(
        noted(&home.log(&cat)),
        ["remember", "clear-memory", "clear-memory"]
    );
    assert_eq!(script.requests().len(), asked, "不请求模型");
}

#[tokio::test]
async fn clear_memory_needs_memory_on() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "/clear-memory").await;
    assert_eq!(reason(&reply), Some("memory_unavailable"), "{reply}");
    assert_eq!(reply["error"]["data"]["why"], "无人格会话不支持记忆。");
    let listed = client
        .call("m1", "command.catalog", json!({"session": session}))
        .await;
    assert!(
        !listed.to_string().contains("clear-memory"),
        "用不了的不列：{listed}"
    );
}

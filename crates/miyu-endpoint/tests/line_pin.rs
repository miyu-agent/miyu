//! 会话钉的压缩触发线在协议上的样子（施工 CT-1，`docs/blueprint/compaction.md` 第二条第 2 条）：`session.configure` 的 `compaction_line`
//! 记一条 `session.policy_changed`（`{"compaction_line":{"percent":…}}`），下一个回合开始时换上：订阅回应的 `limits.compaction_line` 照
//! 新的百分数算；`null` 去掉、回到跟着 `compaction.line`；`view.status` 钉了的带 `compaction_line`，没钉的不写。不在 20 到 95 里的、不是
//! 整数的 `bad_params`，什么都不记。`session.create` 也收：造好以后记一条，回应的 `events` 多它。
//!
//! 替身窗口 100 万、没报最大输出：减法那一条是 967000，85% 是 85 万、50% 是 50 万、60% 是 60 万。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 窗口 100 万、说 `n` 次「好。」的剧本。
fn script(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。"))).window(1_000_000)
}

/// 发一条 `session.configure {session, line}`。
async fn pin(client: &mut Client, id: &str, session: &str, line: Value) -> Value {
    let params = json!({"session": session, "compaction_line": line});
    client.call(id, "session.configure", params).await
}

/// 这一刻订阅回应里的压缩线。
async fn line(client: &mut Client, id: &str, session: &str) -> Value {
    client.subscribe(id, session).await["result"]["limits"]["compaction_line"].clone()
}

/// 说一句，等会话说完第 `turns` 轮。
async fn say(client: &mut Client, home: &Home, session: &str, id: &str, turns: usize) {
    let params = json!({"session": session, "text": "再说一句"});
    let reply = client.call(id, "session.send", params).await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(session, turns).await;
}

/// 日志里钉压缩触发线的那几条的 `body`，照先后。
fn pins(home: &Home, session: &str) -> Vec<Value> {
    logged(home, session)
        .into_iter()
        .filter(|event| {
            event["kind"] == "session.policy_changed"
                && event["body"]["compaction_line"].is_object()
        })
        .map(|event| event["body"].clone())
        .collect()
}

/// 新连上一个头订阅视图流：回应里整份会话状态。
async fn status(core: &Arc<Core>, session: &str) -> Value {
    let mut viewer = Client::connect(Arc::clone(core));
    viewer.hello().await;
    let params = json!({"session": session, "stream": "view"});
    viewer.call("v", "subscribe", params).await["result"]["status"].clone()
}

#[tokio::test]
async fn a_pinned_line_takes_the_next_turn_and_null_follows_the_config() {
    let home = Home::new();
    let core = home.core(&script(3));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "titles": false}))
        .await;
    assert_eq!(line(&mut client, "c2", &session).await, 850_000);
    assert!(
        status(&core, &session)
            .await
            .get("compaction_line")
            .is_none()
    );
    let reply = pin(&mut client, "c3", &session, json!(50)).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(status(&core, &session).await["compaction_line"], 50);
    assert_eq!(
        line(&mut client, "c4", &session).await,
        850_000,
        "没开回合的还是原来的"
    );
    let reply = pin(&mut client, "c5", &session, json!(50)).await;
    assert_eq!(reply["result"], json!({}), "一样的照收：{reply}");
    say(&mut client, &home, &session, "s1", 1).await;
    assert_eq!(line(&mut client, "c6", &session).await, 500_000);
    let reply = pin(&mut client, "c7", &session, Value::Null).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(
        status(&core, &session)
            .await
            .get("compaction_line")
            .is_none()
    );
    say(&mut client, &home, &session, "s2", 2).await;
    assert_eq!(line(&mut client, "c8", &session).await, 850_000);
    assert_eq!(
        pins(&home, &session),
        [
            json!({"compaction_line": {"percent": 50}}),
            json!({"compaction_line": {"percent": null}})
        ],
        "一样的不记"
    );
    let cause = logged(&home, &session)
        .into_iter()
        .find(|event| event["body"]["compaction_line"].is_object())
        .map(|event| event["cause"].clone());
    assert_eq!(cause, Some(json!("c3/compaction_line")));
}

#[tokio::test]
async fn out_of_range_or_not_an_integer_is_refused_and_nothing_is_recorded() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&script(0)));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for (n, line) in [json!(19), json!(96), json!(0)].into_iter().enumerate() {
        let reply = pin(&mut client, &format!("r{n}"), &session, line.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{line}：{reply}");
        assert_eq!(
            reply["error"]["data"]["field"], "compaction_line",
            "{reply}"
        );
    }
    for (n, line) in [json!("50"), json!(50.5), json!(-1)]
        .into_iter()
        .enumerate()
    {
        let reply = pin(&mut client, &format!("b{n}"), &session, line.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{line}：{reply}");
    }
    for (n, line) in [20, 95].into_iter().enumerate() {
        let reply = pin(&mut client, &format!("ok{n}"), &session, json!(line)).await;
        assert_eq!(reply["result"], json!({}), "两头收：{reply}");
    }
    assert_eq!(pins(&home, &session).len(), 2, "拒了的什么都不记");
}

#[tokio::test]
async fn session_create_takes_a_line_as_well() {
    let home = Home::new();
    let core = home.core(&script(1));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let params = json!({"cwd": "~", "titles": false, "compaction_line": 60});
    let reply = client.call("c1", "session.create", params).await;
    assert_eq!(reply["result"]["events"], json!([1, 2]), "{reply}");
    let session = reply["result"]["session"].as_str().expect("造出了会话");
    assert_eq!(
        pins(&home, session),
        [json!({"compaction_line": {"percent": 60}})]
    );
    assert_eq!(status(&core, session).await["compaction_line"], 60);
    say(&mut client, &home, session, "s1", 1).await;
    assert_eq!(
        line(&mut client, "c2", session).await,
        600_000,
        "第一轮就照钉的"
    );
    let made = home.root.sessions().expect("读得了").len();
    for (id, line) in [("c3", json!(96)), ("c4", json!("60"))] {
        let reply = client
            .call(id, "session.create", json!({"compaction_line": line}))
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }
    assert_eq!(
        home.root.sessions().expect("读得了").len(),
        made,
        "不在范围里的什么都不造"
    );
}

/// 事件流订阅的回应顶层也带钉的百分数（施工 CT-1补，终端照它画会话设置）：没钉的不写；钉了以后、核心重启照日志载入以后
/// 都是 50；`limits.compaction_line` 照旧是生效的 token 数，两格同名不同层。带不带 `after` 都一样。
#[tokio::test]
async fn the_events_subscription_carries_the_pinned_line() {
    let home = Home::new();
    let first = home.core(&script(0));
    let mut client = Client::connect(Arc::clone(&first));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.subscribe("c2", &session).await;
    assert!(reply["result"].get("compaction_line").is_none(), "{reply}");
    let reply = pin(&mut client, "c3", &session, json!(50)).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let reply = client.subscribe("c4", &session).await;
    assert_eq!(reply["result"]["compaction_line"], 50, "{reply}");
    assert_eq!(reply["result"]["limits"]["compaction_line"], 850_000);
    first.stop_sessions().await;
    drop(client);
    let mut client = Client::connect(home.core(&script(0)));
    client.hello().await;
    let reply = client.subscribe("c5", &session).await;
    assert_eq!(
        reply["result"]["compaction_line"], 50,
        "重启以后照日志：{reply}"
    );
    let (_, reply) = client.subscribe_after("c6", &session, json!(0)).await;
    assert_eq!(
        reply["result"]["compaction_line"], 50,
        "带 after 的：{reply}"
    );
}

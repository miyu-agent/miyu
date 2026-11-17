//! 场所会话的「查看会话」（施工 VW-2，`docs/construction/VW-2-场所会话的查看会话.md`）：视图流里场所的人说的话 `from` 带
//! `name`；桥分出去的并行支线作为主线会话状态 `jobs` 里的一项（`what: agent`、`fork: true`、`session` 是支线），跑完了照样列、
//! `state` 是 `done`，订阅以后分出去的跟着推；订阅回应、会话状态带标题，改了推一条新的 `view.status`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 主人（`qq:10001`）在 `session` 里说 `text`，平台编号 `msg`，名字「主人」。
fn said(session: &str, text: &str, msg: &str) -> Value {
    json!({"session": session, "text": text, "as": {"external": "qq:10001"},
        "venue": {"msg": msg, "name": "主人"}})
}

/// 从 `session` 分出一条只跑一轮的支线，在里面说 `text`、等它跑完，交回支线的编号。
async fn branch(client: &mut Client, home: &Home, session: &str, text: &str, n: u8) -> String {
    let reply = client
        .call(
            &format!("f{n}"),
            "session.fork",
            json!({"session": session, "seal_after": 1}),
        )
        .await;
    let child = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_string();
    let sent = client
        .call(
            &format!("b{n}"),
            "session.send",
            said(&child, text, &format!("88{n}")),
        )
        .await;
    assert!(sent.get("error").is_none(), "{sent}");
    home.until_turns(&child, 1).await;
    child
}

/// 等到一条标题是 `title` 的 `view.status`，交回那份状态。
async fn titled(client: &mut Client, title: &str) -> Value {
    loop {
        let next = client.next().await.expect("没断开");
        if next["method"] == "view.status" && next["params"]["status"]["title"] == title {
            return next["params"]["status"].clone();
        }
    }
}

#[tokio::test]
async fn a_venue_view_shows_names_branches_and_the_title() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("主线。"),
        Play::Says("支线一。"),
        Play::Says("支线二。"),
        Play::Says("支线三。"),
    ]);
    let core = bound_core(&home, &script, Catalog::default());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001", "titles": false}),
        )
        .await;
    let main = made["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{made}"))
        .to_string();
    let sent = client
        .call("s1", "session.send", said(&main, "在吗", "8810"))
        .await;
    assert!(sent.get("error").is_none(), "{sent}");
    home.until_turns(&main, 1).await;
    let one = branch(&mut client, &home, &main, "支线的第一句", 1).await;
    let two = branch(&mut client, &home, &main, "支线的第二句", 2).await;
    let set = client
        .call(
            "t1",
            "session.set_meta",
            json!({"session": main, "title": "主人的私聊"}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");

    let mut watcher = Client::connect(core);
    watcher.hello().await;
    let reply = watcher
        .call(
            "w1",
            "subscribe",
            json!({"session": main, "stream": "view"}),
        )
        .await;
    let result = &reply["result"];
    assert_eq!(result["title"], "主人的私聊", "订阅回应带标题：{reply}");
    assert_eq!(result["status"]["title"], "主人的私聊", "会话状态带标题");
    let user = result["entries"]
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["kind"] == "user"))
        .unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(user["from"]["name"], "主人", "{user}");
    assert_eq!(user["from"]["via"], "qq:10001", "{user}");
    let branches: Vec<&Value> = result["status"]["jobs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|job| job["fork"] == true)
        .collect();
    let shape: Vec<(&str, &str, &str, &str, &str)> = branches
        .iter()
        .map(|job| {
            let text = |key: &str| job[key].as_str().unwrap_or_default();
            (
                text("job"),
                text("session"),
                text("what"),
                text("state"),
                text("title"),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            (one.as_str(), one.as_str(), "agent", "done", "支线的第一句"),
            (two.as_str(), two.as_str(), "agent", "done", "支线的第二句"),
        ],
        "{reply}"
    );
    assert!(
        branches
            .iter()
            .all(|job| job["started"].is_string() && job["ended"].is_string())
    );

    let renamed = client
        .call(
            "t2",
            "session.set_meta",
            json!({"session": main, "title": "改了名"}),
        )
        .await;
    assert!(renamed.get("error").is_none(), "{renamed}");
    let status = titled(&mut watcher, "改了名").await;
    assert_eq!(status["jobs"].as_array().map(Vec::len), Some(2), "{status}");

    // 订阅以后分出去的：动了就重读，推一份带它的会话状态，跑完是 `done`。
    let three = branch(&mut client, &home, &main, "支线的第三句", 3).await;
    loop {
        let next = watcher.next().await.expect("没断开");
        let done = next["params"]["status"]["jobs"]
            .as_array()
            .is_some_and(|jobs| {
                jobs.iter()
                    .any(|job| job["session"] == three.as_str() && job["state"] == "done")
            });
        if next["method"] == "view.status" && done {
            break;
        }
    }
}

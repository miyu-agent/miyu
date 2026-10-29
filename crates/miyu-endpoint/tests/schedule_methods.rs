//! `schedule.*` 和 `schedules` 流（施工 J-1 下，`docs/blueprint/schedules.md` 第五节）：建、列、停、续、删，一项的写法；写错的
//! 照哪一样拒（`data.problem`），别人的编号、没有的编号 `unknown_schedule`；每次新开的照这时的默认定下人格、预设、工作目录；
//! 订阅的回应是整张表，之后另一个连接改了的、到点交过的推 `schedules.changed`（整项，删了的 `removed`）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::clock::{Hand, core_with};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 拒绝的原因码和 `data.problem`。
fn refused(reply: &Value) -> (&str, &str) {
    (
        reply["error"]["data"]["reason"]
            .as_str()
            .unwrap_or_default(),
        reply["error"]["data"]["problem"]
            .as_str()
            .unwrap_or_default(),
    )
}

/// 读到一条 `schedules.changed` 为止：别的消息跳过，最多 60 秒。
async fn changed(client: &mut Client) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到定时任务的推送"));
        if message["method"] == "schedules.changed" {
            return message["params"].clone();
        }
    }
}

/// 起好唤醒表的核心、握好手的连接、开好的一个会话。
async fn started(
    home: &Home,
    script: &Script,
    hand: &Arc<Hand>,
) -> (Arc<miyu_endpoint::Core>, Client, String) {
    let core = core_with(home, script, hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    core.start_schedules();
    (core, client, session)
}

#[tokio::test]
async fn schedules_are_added_listed_paused_resumed_and_cancelled() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (_core, mut client, session) = started(&home, &Script::new([]), &hand).await;
    let added = client
        .call(
            "a1",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "看看 CI", "target": {"session": session}}),
        )
        .await;
    assert_eq!(
        added["result"]["schedule"],
        json!({
            "id": "t1",
            "when": {"every": 600},
            "prompt": "看看 CI",
            "target": {"session": session},
            "by": {"kind": "person"},
            "recurring": true,
            "paused": false,
            "created": "2026-10-26T00:00:00.000Z",
            "next": "2026-10-26T00:10:00.000Z",
        }),
        "{added}"
    );
    let daily = client
        .call(
            "a2",
            "schedule.add",
            json!({"when": {"cron": "0 8 * * *", "recurring": false}, "prompt": "早报"}),
        )
        .await;
    let daily = &daily["result"]["schedule"];
    assert_eq!(daily["id"], "t2");
    // 没交时区的记这台机器的（施工 J-3 上）。
    let mut when = json!({"cron": "0 8 * * *", "recurring": false});
    if let Some(zone) = jiff::tz::TimeZone::try_system()
        .ok()
        .and_then(|zone| zone.iana_name().map(str::to_string))
    {
        when["zone"] = json!(zone);
    }
    assert_eq!(daily["when"], when);
    assert_eq!(daily["recurring"], false);
    let new = &daily["target"]["new"];
    assert_eq!(new["preset"], "full", "不写的照这时的默认预设：{daily}");
    assert_eq!(new["persona"], Value::Null, "没有默认人格的是无人格");
    assert!(
        new["cwd"].as_str().is_some_and(|cwd| !cwd.is_empty()),
        "工作目录照家目录：{daily}"
    );

    let all = client.call("l1", "schedule.list", json!({})).await;
    let ids: Vec<&str> = all["result"]["schedules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    let on = client
        .call("l2", "schedule.list", json!({"session": session}))
        .await;
    assert_eq!(
        on["result"]["schedules"].as_array().unwrap().len(),
        1,
        "只列跑在它上面的"
    );

    let paused = client
        .call("p", "schedule.pause", json!({"id": "t1"}))
        .await;
    assert_eq!(paused["result"]["schedule"]["paused"], true);
    assert!(
        paused["result"]["schedule"].get("next").is_none(),
        "{paused}"
    );
    hand.advance(60_000);
    let resumed = client
        .call("r", "schedule.resume", json!({"id": "t1"}))
        .await;
    assert_eq!(
        resumed["result"]["schedule"]["next"], "2026-10-26T00:11:00.000Z",
        "照现在重算"
    );
    let cancelled = client
        .call("x", "schedule.cancel", json!({"id": "t1"}))
        .await;
    assert_eq!(cancelled["result"], json!({}));
    for (id, method) in [
        ("x2", "schedule.cancel"),
        ("p2", "schedule.pause"),
        ("r2", "schedule.resume"),
    ] {
        let gone = client.call(id, method, json!({"id": "t1"})).await;
        assert_eq!(refused(&gone).0, "unknown_schedule", "{method}：{gone}");
    }
}

#[tokio::test]
async fn bad_ones_say_what_is_wrong_and_nothing_is_added() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (_core, mut client, session) = started(&home, &Script::new([]), &hand).await;
    let cases = [
        (json!({"when": {}, "prompt": "x"}), "when"),
        (
            json!({"when": {"every": 600, "at": "2026-10-27T00:00:00.000Z"}, "prompt": "x"}),
            "when",
        ),
        (
            json!({"when": {"at": "2026-10-27T00:00:00.000Z", "recurring": true}, "prompt": "x"}),
            "when",
        ),
        (
            json!({"when": {"cron": "0 0 8 * * *"}, "prompt": "x"}),
            "cron",
        ),
        (json!({"when": {"every": 30}, "prompt": "x"}), "short"),
        (
            json!({"when": {"at": "2026-10-25T00:00:00.000Z"}, "prompt": "x"}),
            "past",
        ),
        (json!({"when": {"every": 600}, "prompt": "  "}), "prompt"),
        (
            json!({"when": {"every": 600}, "prompt": "x", "target": {"session": "01a17000-0000-7000-8000-000000000009"}}),
            "session",
        ),
        (
            json!({"when": {"every": 600}, "prompt": "x", "target": {"session": session, "new": {}}}),
            "target",
        ),
    ];
    for (n, (params, problem)) in cases.into_iter().enumerate() {
        let reply = client
            .call(&format!("b{n}"), "schedule.add", params.clone())
            .await;
        assert_eq!(
            refused(&reply),
            ("bad_params", problem),
            "{params}：{reply}"
        );
    }
    let unknown = client
        .call(
            "u",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "x", "target": {"new": {"preset": "nope"}}}),
        )
        .await;
    assert_eq!(refused(&unknown).0, "unknown_preset", "{unknown}");
    let listed = client.call("l", "schedule.list", json!({})).await;
    assert_eq!(listed["result"]["schedules"], json!([]), "什么都没建");
}

#[tokio::test]
async fn subscribers_hear_changes_and_runs() {
    let home = Home::new();
    let hand = Hand::at(START);
    let script = Script::new([Play::Says("看过了。")]);
    let (core, mut client, session) = started(&home, &script, &hand).await;
    client
        .call(
            "a",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "看看", "target": {"session": session}}),
        )
        .await;
    let mut watcher = Client::connect(Arc::clone(&core));
    watcher.hello().await;
    let reply = watcher
        .call("s", "subscribe", json!({"stream": "schedules"}))
        .await;
    assert_eq!(reply["result"]["schedules"][0]["id"], "t1", "{reply}");
    client
        .call(
            "a2",
            "schedule.add",
            json!({"when": {"every": 900}, "prompt": "再看看", "target": {"session": session}}),
        )
        .await;
    let added = changed(&mut watcher).await;
    assert_eq!(
        (&added["id"], &added["entry"]["prompt"]),
        (&json!("t2"), &json!("再看看")),
        "建了的推：{added}"
    );

    client
        .call("p", "schedule.pause", json!({"id": "t1"}))
        .await;
    let pushed = changed(&mut watcher).await;
    assert_eq!(pushed["id"], "t1");
    assert_eq!(pushed["entry"]["paused"], true, "{pushed}");

    client
        .call("r", "schedule.resume", json!({"id": "t1"}))
        .await;
    changed(&mut watcher).await;
    hand.advance(600_000);
    let ran = loop {
        let pushed = changed(&mut watcher).await;
        if pushed["entry"].get("last").is_some() {
            break pushed;
        }
    };
    assert_eq!(
        ran["entry"]["last"]["result"], "ran",
        "到点交过的也推：{ran}"
    );

    client
        .call("x", "schedule.cancel", json!({"id": "t1"}))
        .await;
    assert_eq!(
        changed(&mut watcher).await,
        json!({"id": "t1", "removed": true})
    );
    let stopped = watcher
        .call("u", "unsubscribe", json!({"stream": "schedules"}))
        .await;
    assert_eq!(stopped["result"], json!({}), "{stopped}");
}

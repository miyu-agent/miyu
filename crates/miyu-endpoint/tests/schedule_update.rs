//! `schedule.update`（施工 J-5，`docs/blueprint/schedules.md` 第五节第 3a 条）：改时间、改那句话，编号不变、上一次留着；改了
//! 时间照现在重算下一次，睡着的那个当场照新的醒；停着的改完还停着；循环换不了种类；写错的照 `schedule.add` 的一样拒、什么都
//! 不改；没有的编号 `unknown_schedule`。扩展碰别人建的在 `schedule_package.rs`。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::schedules::{Creator, Loop, Target, Wanted, When};
use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};

use crate::support::clock::{Hand, core_with};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("在时刻的范围里")
}

/// 拒绝的原因码和 `data.problem`（没有的是空的）。
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

/// 列表里编号是 `id` 的那一项。
async fn listed(client: &mut Client, id: &str) -> Value {
    let all = client.call("l", "schedule.list", json!({})).await;
    all["result"]["schedules"]
        .as_array()
        .expect("列表")
        .iter()
        .find(|one| one["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("列表里没有 {id}：{all}"))
}

#[tokio::test]
async fn changing_the_time_recomputes_next_wakes_on_it_and_keeps_the_rest() {
    let home = Home::new();
    let hand = Hand::at(START);
    let script = Script::new([Play::Says("看过了。")]);
    let (core, mut client, session) = started(&home, &script, &hand).await;
    client
        .call(
            "a",
            "schedule.add",
            json!({"when": {"every": 3600}, "prompt": "看看 CI", "target": {"session": session}}),
        )
        .await;
    let mut watcher = Client::connect(Arc::clone(&core));
    watcher.hello().await;
    watcher
        .call("s", "subscribe", json!({"stream": "schedules"}))
        .await;

    hand.advance(60_000);
    let updated = client
        .call(
            "u1",
            "schedule.update",
            json!({"id": "t1", "when": {"every": 120}}),
        )
        .await;
    let entry = &updated["result"]["schedule"];
    assert_eq!(
        entry,
        &json!({
            "id": "t1",
            "when": {"every": 120},
            "prompt": "看看 CI",
            "target": {"session": session},
            "by": {"kind": "person"},
            "recurring": true,
            "paused": false,
            "created": "2026-10-26T00:00:00.000Z",
            "next": "2026-10-26T00:03:00.000Z",
        }),
        "照现在重算下一次，别的不变：{updated}"
    );
    assert_eq!(&listed(&mut client, "t1").await, entry, "列表里是改过的");
    let pushed = changed(&mut watcher).await;
    assert_eq!(pushed, json!({"id": "t1", "entry": entry}), "改了的推");
    // 写进了表：核心重开读得回改过的。
    let (table, _) = miyu_store::schedules::Schedules::open(&home.root);
    let row = table
        .expect("开得了")
        .get("t1")
        .expect("读得了")
        .expect("还在");
    assert_eq!(row.when, When::Every { every: 120 });
    assert_eq!(row.next, Some(at(START + 180_000)));

    // 睡着的那个原来等一小时以后，改完照新的下一次醒。
    hand.advance(120_000);
    let ran = loop {
        let pushed = changed(&mut watcher).await;
        if pushed["entry"].get("last").is_some() {
            break pushed;
        }
    };
    assert_eq!(ran["entry"]["last"]["result"], "ran", "{ran}");

    // 改那句话：编号、上一次都留着，下一次不动。
    let before = listed(&mut client, "t1").await;
    let renamed = client
        .call(
            "u2",
            "schedule.update",
            json!({"id": "t1", "prompt": "再看看 CI"}),
        )
        .await;
    let after = &renamed["result"]["schedule"];
    assert_eq!(after["id"], "t1");
    assert_eq!(after["prompt"], "再看看 CI");
    assert_eq!(after["last"], before["last"], "上一次留着：{renamed}");
    assert_eq!(after["next"], before["next"], "只改话的不重算：{renamed}");
    assert_eq!(after["when"], json!({"every": 120}));
}

#[tokio::test]
async fn a_paused_one_stays_paused_after_an_update() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (_core, mut client, session) = started(&home, &Script::new([]), &hand).await;
    client
        .call(
            "a",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "看看", "target": {"session": session}}),
        )
        .await;
    client
        .call("p", "schedule.pause", json!({"id": "t1"}))
        .await;
    let updated = client
        .call(
            "u",
            "schedule.update",
            json!({"id": "t1", "when": {"cron": "0 8 * * *", "zone": "Asia/Tokyo"}, "prompt": "早报"}),
        )
        .await;
    let entry = &updated["result"]["schedule"];
    assert_eq!(entry["paused"], true, "{updated}");
    assert!(entry.get("next").is_none(), "停着的没有下一次：{updated}");
    assert_eq!(
        entry["when"],
        json!({"cron": "0 8 * * *", "recurring": true, "zone": "Asia/Tokyo"})
    );
    assert_eq!(entry["prompt"], "早报");
    let resumed = client
        .call("r", "schedule.resume", json!({"id": "t1"}))
        .await;
    assert_eq!(
        resumed["result"]["schedule"]["next"], "2026-10-26T23:00:00.000Z",
        "续上照新的时间算：{resumed}"
    );
}

/// 跑在 `session` 上的一个循环，第一次在 `first` 毫秒。
fn looping(session: &str, kind: Loop, when: When, first: i64) -> Wanted {
    Wanted {
        by: Creator::Person,
        when,
        prompt: "看看".to_string(),
        target: Target::Session {
            session: SessionId::parse(session).expect("会话编号合写法"),
        },
        looping: Some(kind),
        first: Some(at(first)),
    }
}

#[tokio::test]
async fn loops_keep_their_kind() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (core, mut client, session) = started(&home, &Script::new([]), &hand).await;
    let dynamic = core
        .schedules()
        .add(looping(
            &session,
            Loop::Dynamic,
            When::At { at: at(START) },
            START + 600_000,
        ))
        .expect("建得了");
    let fixed = core
        .schedules()
        .add(looping(
            &session,
            Loop::Fixed,
            When::Every { every: 600 },
            START + 600_000,
        ))
        .expect("建得了");
    let cases = [
        (&dynamic.id, json!({"every": 600})),
        (&dynamic.id, json!({"cron": "0 8 * * *"})),
        (&fixed.id, json!({"at": "2026-10-27T00:00:00.000Z"})),
        (&fixed.id, json!({"cron": "0 8 * * *"})),
    ];
    for (n, (id, when)) in cases.into_iter().enumerate() {
        let reply = client
            .call(
                &format!("k{n}"),
                "schedule.update",
                json!({"id": id, "when": when}),
            )
            .await;
        assert_eq!(
            refused(&reply),
            ("bad_params", "when"),
            "{id} {when}：{reply}"
        );
    }
    let kept = core.schedules().get(&dynamic.id).expect("还在");
    assert_eq!(kept, dynamic, "拒了的什么都不改");

    let said = client
        .call(
            "d1",
            "schedule.update",
            json!({"id": dynamic.id, "prompt": "再看看"}),
        )
        .await;
    assert_eq!(said["result"]["schedule"]["loop"], "dynamic", "{said}");
    assert_eq!(said["result"]["schedule"]["prompt"], "再看看");
    let later = client
        .call(
            "d2",
            "schedule.update",
            json!({"id": dynamic.id, "when": {"at": "2026-10-26T01:00:00.000Z"}}),
        )
        .await;
    assert_eq!(
        later["result"]["schedule"]["next"], "2026-10-26T01:00:00.000Z",
        "她自己定节奏的改时刻就是改下一次：{later}"
    );
    let slower = client
        .call(
            "f1",
            "schedule.update",
            json!({"id": fixed.id, "when": {"every": 1200}}),
        )
        .await;
    let slower = &slower["result"]["schedule"];
    assert_eq!(slower["loop"], "fixed", "{slower}");
    assert_eq!(slower["next"], "2026-10-26T00:20:00.000Z");
}

#[tokio::test]
async fn bad_updates_are_refused_and_change_nothing() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (_core, mut client, session) = started(&home, &Script::new([]), &hand).await;
    let added = client
        .call(
            "a",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "看看", "target": {"session": session}}),
        )
        .await;
    let cases = [
        (json!({"id": "t1"}), ("bad_params", "")),
        (json!({"id": "t1", "colour": 1}), ("bad_params", "")),
        (json!({"id": "t9", "prompt": "x"}), ("unknown_schedule", "")),
        (
            json!({"id": "t1", "prompt": "  "}),
            ("bad_params", "prompt"),
        ),
        (json!({"id": "t1", "when": {}}), ("bad_params", "when")),
        (
            json!({"id": "t1", "when": {"every": 30}}),
            ("bad_params", "short"),
        ),
        (
            json!({"id": "t1", "when": {"cron": "0 0 8 * * *"}}),
            ("bad_params", "cron"),
        ),
        (
            json!({"id": "t1", "when": {"cron": "0 8 * * *", "zone": "Mars/Base"}}),
            ("bad_params", "zone"),
        ),
        (
            json!({"id": "t1", "when": {"at": "2026-10-25T00:00:00.000Z"}}),
            ("bad_params", "past"),
        ),
        (
            json!({"id": "t1", "when": {"every": 900}, "prompt": ""}),
            ("bad_params", "prompt"),
        ),
    ];
    for (n, (params, want)) in cases.into_iter().enumerate() {
        let reply = client
            .call(&format!("b{n}"), "schedule.update", params.clone())
            .await;
        assert_eq!(refused(&reply), want, "{params}：{reply}");
    }
    assert_eq!(
        listed(&mut client, "t1").await,
        added["result"]["schedule"],
        "拒了的什么都不改"
    );
}

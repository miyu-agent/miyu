//! 扩展建的定时（施工 J-4）：批过 `sessions.drive` 的扩展能调 `schedule.*`，建的记成包建的、只看得到改得了自己建的；「跑在哪」
//! 写 `package` 的到点不进会话，推 `schedule.fired` 给建它的那个扩展，核心停着时错过的几次下一次一起报。人写不了 `package`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_store::schedules::{Creator, NewSchedule, Schedules, Target, When};
use miyu_tool::Catalog;

use crate::support::clock::Hand;
use crate::support::extensions::{Program, approve, install_asking, quick, read, record, steps};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 等到记的文件里有 `want` 那么多行带 `needle` 的，交回这几行（读成 JSON）。
async fn until(path: &Path, needle: &str, want: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let text = read(path);
        let found: Vec<Value> = text
            .lines()
            .filter(|line| line.contains(needle))
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if found.len() >= want {
            return found;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {want} 行 {needle}：{text}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 装上 `id`（声明、批过 `sessions.drive`），照 `asks` 一步步调，最后照 `serve` 记下核心推来的；交回记的文件。
fn install(home: &Home, program: &Program, id: &str, asks: &[&str]) -> std::path::PathBuf {
    let (path, keep) = record(home, id);
    let mut all = vec![keep.as_str(), "hello"];
    all.extend_from_slice(asks);
    all.push("serve");
    install_asking(
        home,
        id,
        &program.name(),
        "always",
        &steps(&all),
        &["sessions.drive"],
    );
    approve(home, id, &["sessions.drive"]);
    path
}

/// 起核心：唤醒表照 `hand` 看时间，扩展、唤醒表都起来。
fn core(home: &Home, hand: &Arc<Hand>) -> Arc<miyu_endpoint::Core> {
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_clock(Arc::clone(hand) as Arc<dyn miyu_endpoint::schedules::Clock>),
    );
    core.start_extensions();
    core.start_schedules();
    core
}

/// 核心起来以前往唤醒表里放一行包 `package` 建的、推给它自己、每十分钟一次，下一次在 `next` 毫秒。
fn added_by(home: &Home, package: &str, next: i64) {
    let (table, _) = Schedules::open(&home.root);
    let table = table.expect("开得了");
    let new = NewSchedule {
        by: Creator::Package {
            package: package.to_string(),
        },
        when: When::Every { every: 600 },
        prompt: "look".to_string(),
        target: Target::Package { missed: 0 },
        looping: None,
        next: miyu_kernel::time::Timestamp::from_unix_millis(next),
    };
    let now =
        miyu_kernel::time::Timestamp::from_unix_millis(START - 3_600_000).expect("在时刻的范围里");
    table.add(new, now).expect("写得进");
}

#[tokio::test]
async fn an_extension_schedules_for_itself_and_sees_only_its_own() {
    let home = Home::new();
    let program = Program::new();
    let go = home.work.join("go");
    let pause = format!("pause:{}", go.display());
    let asks = [
        r#"ask:schedule.add:{"when":{"every":600},"prompt":"{\"group\":1}","target":{"package":{}}}"#,
        &pause,
        "call:schedule.list",
        r#"ask:schedule.cancel:{"id":"t3"}"#,
        r#"ask:schedule.pause:{"id":"t1"}"#,
    ];
    let path = install(&home, &program, "watch", &asks);
    // 别的包建的：它看不到、碰不了。
    added_by(&home, "other", START + 7_200_000);
    let hand = Hand::at(START);
    let core = core(&home, &hand);
    let added = until(&path, r#""id":"s2""#, 1).await;
    let made = &added[0]["result"]["schedule"];
    assert_eq!(made["id"], "t2", "{added:?}");
    assert_eq!(made["by"], json!({"kind": "package", "package": "watch"}));
    assert_eq!(made["target"], json!({"package": {}}));

    // 人：写不了 `package`；建的照旧，看得到全部。
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    let refused = owner
        .call(
            "a1",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "x", "target": {"package": {}}}),
        )
        .await;
    assert_eq!(
        refused["error"]["data"]["reason"], "bad_params",
        "{refused}"
    );
    let cwd = home.work.to_string_lossy().into_owned();
    let mine = owner
        .call(
            "a2",
            "schedule.add",
            json!({"when": {"every": 3600}, "prompt": "早报", "target": {"new": {"cwd": cwd}}}),
        )
        .await;
    assert_eq!(mine["result"]["schedule"]["id"], "t3", "{mine}");
    let all = owner.call("l", "schedule.list", json!({})).await;
    let ids: Vec<&Value> = all["result"]["schedules"]
        .as_array()
        .expect("列表")
        .iter()
        .map(|entry| &entry["id"])
        .collect();
    assert_eq!(ids, [&json!("t2"), &json!("t3"), &json!("t1")], "{all}");

    // 扩展：只看得到自己建的，人建的删不了、别的包建的停不了。
    std::fs::write(&go, "").expect("写得进");
    let listed = until(&path, r#""id":"s4""#, 1).await;
    let listed: Vec<&Value> = listed[0]["result"]["schedules"]
        .as_array()
        .expect("列表")
        .iter()
        .map(|entry| &entry["id"])
        .collect();
    assert_eq!(listed, [&json!("t2")]);
    for step in ["s5", "s6"] {
        let refused = until(&path, &format!(r#""id":"{step}""#), 1).await;
        assert_eq!(
            refused[0]["error"]["data"]["reason"], "unknown_schedule",
            "{refused:?}"
        );
    }
    let kept = owner.call("l2", "schedule.list", json!({})).await;
    assert_eq!(
        kept["result"]["schedules"].as_array().map(Vec::len),
        Some(3),
        "{kept}"
    );
    assert_eq!(kept["result"]["schedules"][2]["paused"], false, "{kept}");

    // 到点推给它自己：编号、该响的时刻、原样的那句话，没错过的不带 `missed`。
    hand.advance(600_000);
    let fired = until(&path, "schedule.fired", 1).await;
    let params = &fired[0]["params"];
    assert_eq!(params["id"], "t2", "{params}");
    assert_eq!(
        params["at"],
        json!(
            miyu_kernel::time::Timestamp::from_unix_millis(START + 600_000)
                .expect("在时刻的范围里")
        )
    );
    assert_eq!(params["prompt"], r#"{"group":1}"#);
    assert!(params.get("missed").is_none(), "{params}");
    let after = owner.call("l3", "schedule.list", json!({})).await;
    assert_eq!(
        after["result"]["schedules"][0]["last"]["result"], "sent",
        "{after}"
    );
}

#[tokio::test]
async fn missed_runs_are_counted_and_told_once() {
    let home = Home::new();
    let program = Program::new();
    let path = install(&home, &program, "watch", &[]);
    // 核心停着：上一次该在半小时以前响，每十分钟一次，错过了四次。
    added_by(&home, "watch", START - 1_800_000);
    let hand = Hand::at(START);
    let _core = core(&home, &hand);
    until(&path, "\"result\"", 1).await;
    hand.advance(600_000);
    let fired = until(&path, "schedule.fired", 1).await;
    assert_eq!(fired[0]["params"]["missed"], 4, "{fired:?}");
    hand.advance(600_000);
    let fired = until(&path, "schedule.fired", 2).await;
    assert!(
        fired[1]["params"].get("missed").is_none(),
        "报过就清零：{fired:?}"
    );
}

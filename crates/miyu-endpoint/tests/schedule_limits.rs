//! 定时任务的时区、上限、工作目录（施工 J-3 上，设计 `45-定时和循环.md`，项目主人 2026-10-27 定）：cron 照建时交的时区读，
//! 不交的记这台机器的；不认识的时区、和 `every` 一起写的拒；全机最多 200 个（人建、她建、包建合计），到了只拒新建，她建的
//! 告诉她叫人删；每次新开的工作目录建时就查，不在的、落在数据根里工作区以外的拒。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::schedules::{Creator, Target, Wanted, When};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};

use crate::support::clock::{Hand, core_with};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z（东京是当天 9 点）。
const START: i64 = 1_792_972_800_000;

/// 拒绝的原因码和 `data.problem`。
fn refused(reply: &Value) -> (String, String) {
    let text = |key: &str| {
        reply["error"]["data"][key]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    (text("reason"), text("problem"))
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
async fn cron_reads_in_the_zone_it_was_given() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (_core, mut client, session) = started(&home, &Script::new([]), &hand).await;
    let tokyo = client
        .call(
            "z1",
            "schedule.add",
            json!({"when": {"cron": "30 7 * * *", "zone": "Asia/Tokyo"}, "prompt": "通勤", "target": {"session": session}}),
        )
        .await;
    let entry = &tokyo["result"]["schedule"];
    assert_eq!(entry["when"]["zone"], "Asia/Tokyo", "{tokyo}");
    assert_eq!(
        entry["next"], "2026-10-26T22:30:00.000Z",
        "东京明早 7 点半：{tokyo}"
    );
    let york = client
        .call(
            "z2",
            "schedule.add",
            json!({"when": {"cron": "30 7 * * *", "zone": "America/New_York"}, "prompt": "通勤", "target": {"session": session}}),
        )
        .await;
    assert_eq!(
        york["result"]["schedule"]["next"], "2026-10-26T11:30:00.000Z",
        "纽约当天 7 点半（夏令时 -4）：{york}"
    );
    let odd = client
        .call(
            "z3",
            "schedule.add",
            json!({"when": {"cron": "30 7 * * *", "zone": "Mars/Base"}, "prompt": "x", "target": {"session": session}}),
        )
        .await;
    assert_eq!(refused(&odd), ("bad_params".into(), "zone".into()), "{odd}");
    let mixed = client
        .call(
            "z4",
            "schedule.add",
            json!({"when": {"every": 600, "zone": "Asia/Tokyo"}, "prompt": "x", "target": {"session": session}}),
        )
        .await;
    assert_eq!(
        refused(&mixed),
        ("bad_params".into(), "when".into()),
        "{mixed}"
    );
}

/// 工具结果的字。
fn results(home: &Home, session: &str) -> Vec<String> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn at_most_two_hundred_in_all() {
    let home = Home::new();
    let resources = default_resources();
    let tools = miyu_tool::Catalog::in_packages([(
        miyu_schedule::SCHEDULE,
        miyu_schedule::schedule_tools(&resources).expect("出厂的字"),
    )])
    .expect("合写法");
    let script = Script::new([
        Play::calls(&[("cron_create", r#"{"cron":"0 9 * * *","prompt":"早"}"#)]),
        Play::Says("好。"),
    ]);
    let core = Arc::new(home.core_full(&script, tools, None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for _ in 0..200 {
        let wanted = Wanted {
            by: Creator::Person,
            when: When::Every { every: 3600 },
            prompt: "看看".to_string(),
            target: Target::Session {
                session: SessionId::parse(&session).expect("合写法"),
            },
            looping: None,
            first: None,
        };
        core.schedules().add(wanted).expect("没满的照建");
    }
    let full = client
        .call(
            "f1",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "再一个", "target": {"session": session}}),
        )
        .await;
    assert_eq!(
        full["error"]["data"]["reason"], "too_many_schedules",
        "{full}"
    );
    assert_eq!(
        core.schedules().list().expect("读得出").len(),
        200,
        "已有的照旧在"
    );

    client.say("s1", &session, "每天九点叫我").await;
    home.until_turns(&session, 1).await;
    let said = results(&home, &session);
    assert!(
        said.iter()
            .any(|text| text.contains("200") && text.contains("safety limit")),
        "她建的告诉她满了：{said:?}"
    );
}

#[tokio::test]
async fn a_new_session_s_folder_is_checked_when_added() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (_core, mut client, _session) = started(&home, &Script::new([]), &hand).await;
    let add = |id: &str, cwd: String| {
        (
            id.to_string(),
            json!({"when": {"every": 600}, "prompt": "x", "target": {"new": {"cwd": cwd}}}),
        )
    };
    let missing = home
        .work
        .join("no-such-folder")
        .to_string_lossy()
        .into_owned();
    let elsewhere = home
        .root
        .path()
        .to_path_buf()
        .to_string_lossy()
        .into_owned();
    let fine = home.work.to_string_lossy().into_owned();
    for (cwd, wanted) in [(missing, "cwd"), (elsewhere, "cwd")] {
        let (id, params) = add("n1", cwd.clone());
        let reply = client.call(&id, "schedule.add", params).await;
        assert_eq!(
            refused(&reply),
            ("bad_params".into(), wanted.into()),
            "{cwd}：{reply}"
        );
    }
    let (id, params) = add("n2", fine);
    let reply = client.call(&id, "schedule.add", params).await;
    assert!(
        reply["result"]["schedule"].is_object(),
        "在的目录照建：{reply}"
    );
}

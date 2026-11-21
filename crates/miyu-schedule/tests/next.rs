//! `schedule_next`（施工 J-2）：间隔收边界、原话覆盖、停、没给间隔、没开过循环、没有端口、给人看的说法带理由。

use serde_json::json;

use miyu_tool::{ScheduleAnswer, ScheduleAsk, ScheduleRefused, Scheduled};

use crate::support::*;

fn woken() -> ScheduleAnswer {
    ScheduleAnswer::Woken(Scheduled {
        kind: Some("dynamic"),
        recurring: false,
        ..item("t5", "at", "盯 CI")
    })
}

#[tokio::test]
async fn the_delay_is_kept_within_bounds() {
    for (given, kept) in [
        (5, 60),
        (-3, 60),
        (600, 600),
        (99_999, 99_999),
        (i64::MAX, i64::MAX as u64),
    ] {
        let port = Fake::answering(Ok(woken()));
        run(
            "schedule_next",
            Some(&port),
            json!({"delay_seconds": given, "reason": "x"}),
        )
        .await;
        assert_eq!(
            port.asked(),
            [(
                session(),
                ScheduleAsk::Wakeup {
                    delay: kept,
                    prompt: None
                }
            )],
            "{given} 秒"
        );
    }
}

#[tokio::test]
async fn a_wakeup_says_when_and_shows_the_reason() {
    let port = Fake::answering(Ok(woken()));
    let done = run(
        "schedule_next",
        Some(&port),
        json!({"delay_seconds": 1200, "prompt": "盯 CI", "reason": "CI 大约二十分钟"}),
    )
    .await;
    assert!(!done.error);
    assert_eq!(text(&done), "Next iteration at 2026-10-27 09:00.");
    assert_eq!(
        port.asked()[0].1,
        ScheduleAsk::Wakeup {
            delay: 1_200,
            prompt: Some("盯 CI".to_string()),
        }
    );
    assert_eq!(
        said(&done),
        (
            "software/loop/next/set".to_string(),
            vec![
                ("next".to_string(), "2026-10-27 09:00".to_string()),
                ("reason".to_string(), "CI 大约二十分钟".to_string()),
            ]
        )
    );
}

/// 施工 WD-6：端口答的那一项没有下一次的时刻（真端口不会这样答），是答错了，照用不了说，不说成「下次 」后面空着。
#[tokio::test]
async fn a_wakeup_without_a_time_is_not_said_as_set() {
    let port = Fake::answering(Ok(ScheduleAnswer::Woken(Scheduled {
        kind: Some("dynamic"),
        recurring: false,
        next: None,
        ..item("t5", "at", "盯 CI")
    })));
    let done = run(
        "schedule_next",
        Some(&port),
        json!({"delay_seconds": 1200, "reason": "x"}),
    )
    .await;
    assert!(done.error, "{}", text(&done));
    assert_eq!(said(&done).0, "software/loop/common/off");
}

#[tokio::test]
async fn a_blank_prompt_keeps_the_old_one() {
    let port = Fake::answering(Ok(woken()));
    run(
        "schedule_next",
        Some(&port),
        json!({"delay_seconds": 60, "prompt": " "}),
    )
    .await;
    assert_eq!(
        port.asked()[0].1,
        ScheduleAsk::Wakeup {
            delay: 60,
            prompt: None
        }
    );
}

#[tokio::test]
async fn stop_ends_the_loop_and_ignores_the_rest() {
    let port = Fake::answering(Ok(ScheduleAnswer::LoopStopped));
    let done = run(
        "schedule_next",
        Some(&port),
        json!({"stop": true, "delay_seconds": 5}),
    )
    .await;
    assert!(!done.error);
    assert_eq!(text(&done), "Loop stopped.");
    assert_eq!(port.asked(), [(session(), ScheduleAsk::StopLoop)]);
}

#[tokio::test]
async fn without_a_delay_it_asks_for_one() {
    let port = Fake::answering(Ok(woken()));
    let done = run("schedule_next", Some(&port), json!({"reason": "x"})).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "delay_seconds is required unless stop is true."
    );
    assert!(port.asked().is_empty());
}

#[tokio::test]
async fn with_no_loop_running_it_says_so() {
    let port = Fake::answering(Err(ScheduleRefused::NoLoop));
    let done = run("schedule_next", Some(&port), json!({"delay_seconds": 60})).await;
    assert!(done.error);
    assert_eq!(text(&done), "No loop is running in this session.");
    assert_eq!(said(&done).0, "software/loop/next/no-loop");
}

#[tokio::test]
async fn without_the_port_it_is_not_available() {
    let done = run("schedule_next", None, json!({"delay_seconds": 60})).await;
    assert!(done.error);
    assert_eq!(text(&done), "Loops are not available in this session.");
}

/// 施工 WK-2：循环这个包只有一件，叫 `schedule_next`；旧名 `schedule_wakeup` 不留。
#[test]
fn the_loop_package_s_only_tool_is_schedule_next() {
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let names: Vec<String> = miyu_schedule::loop_tools(&resources)
        .expect("读得出")
        .iter()
        .map(|tool| tool.spec().name.clone())
        .collect();
    assert_eq!(names, ["schedule_next"]);
}

//! `cron_create`、`cron_delete`、`cron_list`（施工 J-2）：交给端口的、结果的句子逐字节比、端口拒的几种、参数不对、没有端口、
//! 给人看的说法。

use serde_json::json;

use miyu_tool::{ScheduleAnswer, ScheduleAsk, ScheduleRefused, Scheduled};

use crate::support::*;

#[tokio::test]
async fn create_hands_the_job_to_this_session_and_says_when_it_runs() {
    let port = Fake::answering(Ok(ScheduleAnswer::Created(item("t3", "0 9 * * *", "早报"))));
    let done = run(
        "cron_create",
        Some(&port),
        json!({"cron": " 0 9 * * * ", "prompt": "早报"}),
    )
    .await;
    assert!(!done.error);
    assert_eq!(text(&done), "Scheduled t3. Next run: 2026-10-27 09:00.");
    assert_eq!(
        port.asked(),
        [(
            session(),
            ScheduleAsk::Create {
                cron: "0 9 * * *".to_string(),
                prompt: "早报".to_string(),
                recurring: true,
            }
        )],
        "不写 recurring 的是循环，cron 去掉两头空白"
    );
    assert_eq!(said(&done).0, "software/schedule/cron/created");
}

#[tokio::test]
async fn a_one_off_job_says_so() {
    let port = Fake::answering(Ok(ScheduleAnswer::Created(item(
        "t1",
        "30 14 27 10 *",
        "看部署",
    ))));
    run(
        "cron_create",
        Some(&port),
        json!({"cron": "30 14 27 10 *", "prompt": "看部署", "recurring": false}),
    )
    .await;
    assert!(matches!(
        &port.asked()[0].1,
        ScheduleAsk::Create {
            recurring: false,
            ..
        }
    ));
}

#[tokio::test]
async fn an_empty_prompt_never_reaches_the_table() {
    let port = Fake::answering(Ok(ScheduleAnswer::Listed(Vec::new())));
    let done = run(
        "cron_create",
        Some(&port),
        json!({"cron": "* * * * *", "prompt": "  "}),
    )
    .await;
    assert!(done.error);
    assert_eq!(text(&done), "The arguments are not right: prompt is empty.");
    assert!(port.asked().is_empty());
}

#[tokio::test]
async fn a_full_session_and_a_bad_cron_say_why() {
    let full = Fake::answering(Err(ScheduleRefused::Full));
    let done = run(
        "cron_create",
        Some(&full),
        json!({"cron": "* * * * *", "prompt": "x"}),
    )
    .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "This session already has 20 jobs. Delete one with cron_delete first."
    );
    let bad = Fake::answering(Err(ScheduleRefused::BadCron("6 fields".to_string())));
    let done = run(
        "cron_create",
        Some(&bad),
        json!({"cron": "* * * * * *", "prompt": "x"}),
    )
    .await;
    assert_eq!(text(&done), "The cron expression is not valid: 6 fields.");
    assert_eq!(said(&done).0, "software/schedule/cron/bad-cron");
}

#[tokio::test]
async fn delete_names_the_job_either_way() {
    let port = Fake::answering(Ok(ScheduleAnswer::Deleted(item("t2", "0 9 * * *", "x"))));
    let done = run("cron_delete", Some(&port), json!({"id": " t2 "})).await;
    assert_eq!(text(&done), "Deleted t2.");
    assert_eq!(
        port.asked()[0].1,
        ScheduleAsk::Delete {
            id: "t2".to_string()
        }
    );
    let missing = Fake::answering(Err(ScheduleRefused::NotFound));
    let done = run("cron_delete", Some(&missing), json!({"id": "t9"})).await;
    assert!(done.error);
    assert_eq!(text(&done), "There is no job t9 in this session.");
}

#[tokio::test]
async fn list_writes_one_line_a_job() {
    let once = Scheduled {
        recurring: false,
        ..item("t1", "30 14 27 10 *", "看部署")
    };
    let fixed = Scheduled {
        kind: Some("fixed"),
        ..item("t2", "every 300s", "看 CI")
    };
    let stopped = Scheduled {
        kind: Some("dynamic"),
        next: None,
        ..item("t3", "at", "盯日志")
    };
    let paused = Scheduled {
        paused: true,
        ..item("t4", "0 9 * * 1-5", "早报")
    };
    let port = Fake::answering(Ok(ScheduleAnswer::Listed(vec![
        once, fixed, stopped, paused,
    ])));
    let done = run("cron_list", Some(&port), json!({})).await;
    assert_eq!(
        text(&done),
        "t1 | 30 14 27 10 * | once | next: 2026-10-27 09:00 | 看部署\n\
         t2 | every 300s | loop:fixed | next: 2026-10-27 09:00 | 看 CI\n\
         t3 | at | loop:dynamic | next: stopped | 盯日志\n\
         t4 | 0 9 * * 1-5 | recurring | next: paused | 早报"
    );
    assert_eq!(port.asked(), [(session(), ScheduleAsk::List)]);
    assert_eq!(
        said(&done),
        (
            "software/schedule/cron/listed".to_string(),
            vec![("count".to_string(), "4".to_string())]
        )
    );
    let empty = Fake::answering(Ok(ScheduleAnswer::Listed(Vec::new())));
    let done = run("cron_list", Some(&empty), json!({})).await;
    assert_eq!(text(&done), "No jobs are scheduled in this session.");
}

#[tokio::test]
async fn without_the_port_it_is_not_available() {
    for (name, args) in [
        ("cron_create", json!({"cron": "* * * * *", "prompt": "x"})),
        ("cron_delete", json!({"id": "t1"})),
        ("cron_list", json!({})),
    ] {
        let done = run(name, None, args).await;
        assert!(done.error, "{name}");
        assert_eq!(
            text(&done),
            "Scheduled jobs are not available in this session."
        );
    }
    let unavailable = Fake::answering(Err(ScheduleRefused::Unavailable));
    let done = run("cron_list", Some(&unavailable), json!({})).await;
    assert_eq!(
        text(&done),
        "Scheduled jobs are not available in this session."
    );
}

#[tokio::test]
async fn bad_arguments_say_what_is_wrong() {
    let port = Fake::answering(Ok(ScheduleAnswer::Listed(Vec::new())));
    let done = run("cron_create", Some(&port), json!({"prompt": "x"})).await;
    assert!(done.error);
    assert!(
        text(&done).starts_with("The arguments are not right: missing field `cron`"),
        "{}",
        text(&done)
    );
    assert!(port.asked().is_empty());
}

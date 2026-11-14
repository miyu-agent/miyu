//! `end_life`（施工 LF-4 下，`docs/blueprint/tools/end_life.md`）：假端口。遗言原样交给端口，成了、已经结束了、不是人生会话、用不了
//! 各回各的一句；没写、空白、参数读不出来的不交端口，回同一句；没有端口的照不是人生会话说。真端口在协议端点里测
//! （`crates/miyu-endpoint/tests/life_self_end.rs`）。

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::id::{CallId, SessionId};
use miyu_tool::{
    Done, LifeEnding, LifePort, LifeRefused, Progress, ScheduleAnswering, ScheduleAsk,
    SchedulePort, ScheduleRefused,
};

use crate::support::{ids, said, session, text};

/// 假的定时端口，带一个假的人生端口：记下交来的，照预先定好的交回。
struct Fake {
    asked: Mutex<Vec<(SessionId, CallId, String)>>,
    answer: Result<(), LifeRefused>,
}

impl SchedulePort for Fake {
    fn schedule(&self, _session: &SessionId, _ask: ScheduleAsk) -> ScheduleAnswering<'_> {
        Box::pin(async { Err(ScheduleRefused::Unavailable) })
    }

    fn life(&self) -> Option<&dyn LifePort> {
        Some(self)
    }
}

impl LifePort for Fake {
    fn end(&self, session: &SessionId, call: &CallId, last_words: String) -> LifeEnding<'_> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session.clone(), *call, last_words));
        let answer = self.answer;
        Box::pin(async move { answer })
    }
}

fn fake(answer: Result<(), LifeRefused>) -> Arc<Fake> {
    Arc::new(Fake {
        asked: Mutex::default(),
        answer,
    })
}

/// 用端口 `port`（没有的是这次调用没有定时端口）调一次。
async fn run(port: Option<Arc<dyn SchedulePort>>, args: &str) -> Done {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = miyu_schedule::life_tools(&resources).expect("读得出");
    assert_eq!(tools.len(), 1);
    let mut call = miyu_tool::testkit::call(args);
    call.schedules = port;
    call.ids = Some(ids());
    tools[0].run(call, Progress::new(|_| {})).await
}

#[tokio::test]
async fn her_last_words_go_to_the_port_as_written() {
    let port = fake(Ok(()));
    let words = "  谢谢你。\n再见。 ";
    let done = run(
        Some(Arc::clone(&port) as Arc<dyn SchedulePort>),
        &json!({"last_words": words}).to_string(),
    )
    .await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "You have died. This was your last moment awake."
    );
    assert_eq!(said(&done), ("software/life/end/ended".to_string(), vec![]));
    let asked = port
        .asked
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    assert_eq!(asked, [(session(), ids().call, words.to_string())]);
}

#[tokio::test]
async fn missing_or_blank_last_words_are_refused_before_the_port() {
    let port = fake(Ok(()));
    for args in [
        "{}",
        r#"{"last_words": ""}"#,
        r#"{"last_words": " \n\t "}"#,
        r#"{"last_words": 3}"#,
        "[]",
    ] {
        let done = run(Some(Arc::clone(&port) as Arc<dyn SchedulePort>), args).await;
        assert!(done.error, "{args}");
        assert_eq!(text(&done), "last_words is required.", "{args}");
        assert_eq!(said(&done).0, "software/life/end/required");
    }
    assert!(
        port.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty(),
        "什么都没交"
    );
}

#[tokio::test]
async fn refusals_are_said_in_their_own_words() {
    let cases = [
        (
            LifeRefused::Ended,
            "This life has already ended.",
            false,
            "already",
        ),
        (
            LifeRefused::NotLife,
            "This is not a life session.",
            true,
            "not-life",
        ),
        (
            LifeRefused::Unavailable,
            "This is not a life session.",
            true,
            "not-life",
        ),
    ];
    for (refused, line, error, key) in cases {
        let port = fake(Err(refused));
        let done = run(
            Some(port as Arc<dyn SchedulePort>),
            r#"{"last_words":"再见"}"#,
        )
        .await;
        assert_eq!(text(&done), line, "{refused:?}");
        assert_eq!(done.error, error, "已经结束了的不报错：{refused:?}");
        assert_eq!(said(&done).0, format!("software/life/end/{key}"));
    }
}

#[tokio::test]
async fn without_a_life_port_it_is_not_a_life_session() {
    let done = run(None, r#"{"last_words":"再见"}"#).await;
    assert!(done.error);
    assert_eq!(text(&done), "This is not a life session.");
    let only_schedules = crate::support::Fake::answering(Err(ScheduleRefused::Unavailable));
    let done = run(
        Some(only_schedules as Arc<dyn SchedulePort>),
        r#"{"last_words":"再见"}"#,
    )
    .await;
    assert_eq!(
        text(&done),
        "This is not a life session.",
        "定时端口不带人生端口的同样"
    );
}

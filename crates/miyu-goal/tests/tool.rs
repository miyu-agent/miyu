//! `goal`（施工 X-8 上，`docs/blueprint/tools/goal.md`）：四个动作交给端口的、结果的句子逐字节比、续轮里的收尾、端口拒的几种、
//! 参数不对、没有端口、给人看的说法。端口是假的：真的在 `miyu-session` 里测。

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::Block;
use miyu_kernel::session::Reason;
use miyu_tool::{
    Done, GoalAnswering, GoalAsk, GoalNow, GoalPort, GoalRefused, GoalSeen, Progress, Tool,
};

/// 一个假端口：记下她要做的，照预先定好的交回。
struct Fake {
    asked: Mutex<Vec<GoalAsk>>,
    answer: Result<GoalSeen, GoalRefused>,
}

impl Fake {
    fn answering(answer: Result<GoalSeen, GoalRefused>) -> Arc<Fake> {
        Arc::new(Fake {
            asked: Mutex::default(),
            answer,
        })
    }

    fn asked(&self) -> Vec<GoalAsk> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl GoalPort for Fake {
    fn goal(&self, ask: GoalAsk) -> GoalAnswering<'_> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(ask);
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
}

fn tool() -> Arc<dyn Tool> {
    miyu_goal::tools(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
        .expect("读得出")
        .remove(0)
}

/// 用假端口 `port`（没有的是这个会话用不了）调一次。
async fn call(port: Option<&Arc<Fake>>, args: serde_json::Value) -> Done {
    let mut call = miyu_tool::testkit::call(&args.to_string());
    call.goals = port.map(|port| Arc::clone(port) as Arc<dyn GoalPort>);
    tool().run(call, Progress::new(|_| {})).await
}

fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

fn said(done: &Done) -> (String, Vec<(String, String)>) {
    let said = done.human.as_ref().expect("有给人看的说法");
    let fields = said
        .fields
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    (said.key.clone(), fields)
}

/// 跑着的目标：原文带引号和换行，第 `round` 轮。
fn running(round: u32, in_round: bool) -> GoalSeen {
    GoalSeen {
        now: Some(GoalNow {
            text: "修绿 \"CI\"\n再发版".to_string(),
            limit: None,
            state: "running",
            reason: None,
            round,
        }),
        in_round,
    }
}

#[tokio::test]
async fn it_is_named_goal_and_only_reads() {
    let tool = tool();
    assert_eq!(tool.spec().name, "goal");
    assert_eq!(tool.spec().access, miyu_kernel::tool::Access::Read);
}

#[tokio::test]
async fn get_shows_the_goal_with_its_round_and_reason() {
    let mut seen = running(3, false);
    if let Some(goal) = seen.now.as_mut() {
        goal.limit = NonZeroU32::new(10);
        goal.state = "blocked";
        goal.reason = Some("缺 key".to_string());
    }
    let port = Fake::answering(Ok(seen));
    let done = call(Some(&port), serde_json::json!({"action": "get"})).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "Objective: 修绿 \\u0022CI\\u0022\\n再发版\nState: blocked · Round 3 of 10\nBlocked: 缺 key"
    );
    assert_eq!(port.asked(), [GoalAsk::Get]);
    assert_eq!(said(&done).0, "software/goal/goal/now");
}

#[tokio::test]
async fn get_without_a_limit_or_a_goal() {
    let port = Fake::answering(Ok(running(0, false)));
    let done = call(Some(&port), serde_json::json!({"action": "get"})).await;
    assert_eq!(
        text(&done),
        "Objective: 修绿 \\u0022CI\\u0022\\n再发版\nState: running · Round 0"
    );
    let port = Fake::answering(Ok(GoalSeen {
        now: None,
        in_round: false,
    }));
    let done = call(Some(&port), serde_json::json!({"action": "get"})).await;
    assert!(!done.error);
    assert_eq!(text(&done), "No goal is set.");
    assert_eq!(said(&done).0, "software/goal/goal/none");
}

#[tokio::test]
async fn set_hands_the_objective_and_limit_to_the_port() {
    let port = Fake::answering(Ok(GoalSeen {
        now: None,
        in_round: false,
    }));
    let args = serde_json::json!({"action": "set", "objective": "把测试修绿", "max_rounds": 12});
    let done = call(Some(&port), args).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "Goal set. A new round now starts by itself whenever the session is idle."
    );
    assert_eq!(
        port.asked(),
        [GoalAsk::Set {
            text: "把测试修绿".to_string(),
            limit: NonZeroU32::new(12),
        }]
    );
    assert_eq!(
        said(&done),
        (
            "software/goal/goal/set".to_string(),
            vec![("objective".to_string(), "把测试修绿".to_string())]
        )
    );
    // 不写上限的不限。
    call(
        Some(&port),
        serde_json::json!({"action": "set", "objective": "x"}),
    )
    .await;
    assert_eq!(
        port.asked()[1],
        GoalAsk::Set {
            text: "x".to_string(),
            limit: None,
        }
    );
}

#[tokio::test]
async fn ending_in_a_goal_round_carries_the_wrapup() {
    let port = Fake::answering(Ok(running(4, true)));
    let done = call(Some(&port), serde_json::json!({"action": "complete"})).await;
    assert!(!done.error);
    let text = text(&done);
    assert!(
        text.starts_with(
            "<goal_complete>\nObjective: 修绿 \\u0022CI\\u0022\\n再发版\nThe goal is marked complete"
        ),
        "{text}"
    );
    assert!(
        text.ends_with("next instruction.\n</goal_complete>"),
        "{text}"
    );
    assert_eq!(said(&done).0, "software/goal/goal/completed");
    let args = serde_json::json!({"action": "blocked", "reason": "缺 \"key\""});
    let done = call(Some(&port), args).await;
    let text = self::text(&done);
    assert!(
        text.starts_with(
            "<goal_blocked>\nObjective: 修绿 \\u0022CI\\u0022\\n再发版\nBlocked: 缺 \\u0022key\\u0022\nThe goal is marked blocked"
        ),
        "{text}"
    );
    assert!(text.ends_with("</goal_blocked>"), "{text}");
    assert_eq!(
        port.asked()[1],
        GoalAsk::Blocked {
            reason: "缺 \"key\"".to_string()
        }
    );
    assert_eq!(
        said(&done),
        (
            "software/goal/goal/blocked".to_string(),
            vec![("reason".to_string(), "缺 \"key\"".to_string())]
        )
    );
}

#[tokio::test]
async fn ending_in_a_turn_the_person_started_just_says_so() {
    let port = Fake::answering(Ok(running(1, false)));
    let done = call(Some(&port), serde_json::json!({"action": "complete"})).await;
    assert_eq!(text(&done), "Goal marked complete.");
    let args = serde_json::json!({"action": "blocked", "reason": "r"});
    let done = call(Some(&port), args).await;
    assert_eq!(text(&done), "Goal marked blocked.");
}

#[tokio::test]
async fn refusals_say_what_went_wrong() {
    let cases = [
        (
            GoalRefused::TooEarly { round: 1 },
            "Not marked blocked: this is round 1, and blocked needs at least 3 goal rounds.",
            "software/goal/goal/too-early",
        ),
        (
            GoalRefused::Kernel(Reason::GoalExists),
            "A goal is already set, readable with action get. Only the user can change or clear it, with /goal.",
            "software/goal/goal/exists",
        ),
        (
            GoalRefused::Kernel(Reason::NoGoal),
            "No goal is set.",
            "software/goal/goal/none",
        ),
        (
            GoalRefused::Kernel(Reason::EmptyMessage),
            "The goal was not changed: empty_message.",
            "software/goal/goal/failed",
        ),
        (
            GoalRefused::Stopped,
            "The goal was not changed: stopped.",
            "software/goal/goal/failed",
        ),
    ];
    for (refused, expected, key) in cases {
        let port = Fake::answering(Err(refused));
        let args = serde_json::json!({"action": "blocked", "reason": "r"});
        let done = call(Some(&port), args).await;
        assert!(done.error, "{refused:?}");
        assert_eq!(text(&done), expected);
        assert_eq!(said(&done).0, key);
    }
}

#[tokio::test]
async fn bad_arguments_and_no_port() {
    let port = Fake::answering(Ok(running(1, false)));
    for args in [
        serde_json::json!({"action": "set"}),
        serde_json::json!({"action": "blocked"}),
        serde_json::json!({"action": "pause"}),
        serde_json::json!({"action": "set", "objective": "x", "max_rounds": 0}),
        serde_json::json!({}),
    ] {
        let done = call(Some(&port), args.clone()).await;
        assert!(done.error, "{args}");
        assert!(
            text(&done).starts_with("Bad arguments: "),
            "{args}: {}",
            text(&done)
        );
        assert_eq!(said(&done).0, "software/goal/common/bad-args");
    }
    assert!(port.asked().is_empty(), "参数不对的不交端口");
    let done = call(None, serde_json::json!({"action": "get"})).await;
    assert!(done.error);
    assert_eq!(text(&done), "Goals are not available in this session.");
    assert_eq!(said(&done).0, "software/goal/common/off");
}

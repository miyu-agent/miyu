//! 长期目标的工具（施工 X-8 上，`docs/blueprint/tools/goal.md`）：真 actor、假模型、真的 `goal`。她自己设的目标当场跑起来，
//! 续轮消息说是她设的；续轮里报卡住，不到第 3 轮拒、到了第 3 轮成，结果带收尾，之后不再开；报完成同样；已经有一个时再设拒，
//! 看的照会话状态答；只有本机的主会话的工具面上有它。

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, ToolStatus};
use miyu_kernel::id::{SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, GoalChange};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Goal, GoalState, Handle, Lineage};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 有真的 `goal`、一件只读的假工具 `read` 的目录。
fn catalog() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut tools = miyu_goal::tools(&resources).expect("读得出");
    tools.push(Fake::new("read", Access::Read, Act::Echo) as Arc<dyn Tool>);
    Catalog::new(tools).expect("合写法")
}

/// 照 `plays` 回，起标题的请求回一个标题。
fn script(plays: impl IntoIterator<Item = Play>) -> Script {
    Script::new(plays).titles([Play::Says("测试")])
}

/// 等到目标合 `wanted`：最多六十秒，等不到的印出最后看到的目标。
async fn until_goal(handle: &Handle, wanted: impl Fn(&Goal) -> bool) -> Goal {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let current = handle.current().await.expect("会话在跑");
        match current.goal {
            Some(goal) if wanted(&goal) => return goal,
            last => assert!(
                tokio::time::Instant::now() < deadline,
                "六十秒内没等到目标，最后是：{last:?}"
            ),
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// 等到日志里结束了 `turns` 轮：最多六十秒。
async fn until_ended(home: &Home, session: &SessionId, turns: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while ended(home, session) < turns {
        assert!(
            tokio::time::Instant::now() < deadline,
            "六十秒内没等到第 {turns} 轮结束"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn ended(home: &Home, session: &SessionId) -> usize {
    home.log(session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .count()
}

/// 日志里的续轮消息，照先后。
fn rounds(home: &Home, session: &SessionId) -> Vec<String> {
    home.log(session)
        .into_iter()
        .filter(|event| matches!(&event.by, By::Module(module) if module.id.as_str() == "goal"))
        .filter_map(|event| match event.body {
            Body::MessageUser(message) => message.blocks.into_iter().find_map(text),
            _ => None,
        })
        .collect()
}

/// 日志里的工具结果：成没成、字，照先后。等到目标结束了、开了的回合都结束了才看：日志一批批落盘，比会话里目标的状态晚
/// 一点（CI run 1898 的 Windows 上读早了，最后那件的结果还没落盘）。
async fn results(home: &Home, session: &SessionId) -> Vec<(bool, String)> {
    let log = until_logged(home, session, |log| {
        let count = |kind: fn(&Body) -> bool| log.iter().filter(|event| kind(&event.body)).count();
        log.iter()
            .any(|event| matches!(event.body, Body::GoalEnded(_)))
            && count(|body| matches!(body, Body::TurnStarted(_)))
                == count(|body| matches!(body, Body::TurnEnded(_)))
    })
    .await;
    log.into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some((
                result.status == ToolStatus::Ok,
                result.blocks.into_iter().filter_map(text).collect(),
            )),
            _ => None,
        })
        .collect()
}

fn text(block: Block) -> Option<String> {
    match block {
        Block::Text(text) => Some(text.text),
        _ => None,
    }
}

#[tokio::test]
async fn she_sets_a_goal_and_blocked_waits_for_round_three() {
    let home = Home::new();
    let set = r#"{"action":"set","objective":"把测试修绿"}"#;
    let blocked = r#"{"action":"blocked","reason":"缺 key"}"#;
    let models = script([
        // 人开的一轮：她自己设目标。
        Play::calls(&[("goal", set)]),
        Play::Says("开始做。"),
        // 第 1 轮：报卡住太早。
        Play::calls(&[("goal", blocked)]),
        Play::Says("那我接着做。"),
        // 第 2 轮：做一步。
        Play::calls(&[("read", "{}")]),
        Play::Says("读完了。"),
        // 第 3 轮：报卡住成了。
        Play::calls(&[("goal", blocked)]),
        Play::Says("卡在缺 key，要你给一个。"),
    ]);
    let handle = home.create_with(&models, &catalog()).await;
    let session = handle.id().clone();
    ask(&handle, "m1", say("帮我把测试修绿"))
        .await
        .expect("会话在跑");
    let goal = until_goal(&handle, |goal| goal.state == GoalState::Blocked).await;
    assert_eq!(
        (goal.round, goal.reason.as_deref(), goal.by_you),
        (3, Some("缺 key"), true)
    );
    until_ended(&home, &session, 4).await;
    let set_by = home
        .log(&session)
        .into_iter()
        .find_map(|event| matches!(event.body, Body::GoalSet(_)).then_some(event.by));
    assert!(matches!(set_by, Some(By::Tool(_))), "{set_by:?}");
    let sent = rounds(&home, &session);
    assert_eq!(sent.len(), 3, "{sent:?}");
    assert!(
        sent[0].contains("Your standing objective, which you set yourself with the goal tool."),
        "{}",
        sent[0]
    );
    assert!(
        sent[1].contains("— your standing objective, which you set yourself with the goal tool:"),
        "{}",
        sent[1]
    );
    let results = results(&home, &session).await;
    assert_eq!(results.len(), 4, "{results:?}");
    assert_eq!(
        results[0],
        (
            true,
            "Goal set. From now on a new round starts by itself whenever the session is idle."
                .to_string()
        )
    );
    assert_eq!(
        results[1],
        (
            false,
            "Not marked blocked: this is round 1, and blocked needs at least 3 goal rounds. Make another concrete step first."
                .to_string()
        )
    );
    assert!(
        results[3].0
            && results[3]
                .1
                .starts_with("<goal_blocked>\nObjective: 把测试修绿\nBlocked: 缺 key\n"),
        "{:?}",
        results[3]
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(ended(&home, &session), 4, "卡住以后不再开");
}

#[tokio::test]
async fn blocked_in_a_turn_the_person_started_skips_the_three_rounds() {
    let home = Home::new();
    let models = script([
        Play::calls(&[("goal", r#"{"action":"set","objective":"把测试修绿"}"#)]),
        Play::calls(&[("goal", r#"{"action":"blocked","reason":"缺 key"}"#)]),
        Play::Says("卡在缺 key。"),
    ]);
    let handle = home.create_with(&models, &catalog()).await;
    let session = handle.id().clone();
    ask(&handle, "m1", say("帮我把测试修绿"))
        .await
        .expect("会话在跑");
    let goal = until_goal(&handle, |goal| goal.state == GoalState::Blocked).await;
    assert_eq!(goal.round, 0);
    until_ended(&home, &session, 1).await;
    let results = results(&home, &session).await;
    assert_eq!(
        results[1],
        (true, "Goal marked blocked.".to_string()),
        "人在场：不带收尾"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(rounds(&home, &session).is_empty(), "卡住了，不开续轮");
}

#[tokio::test]
async fn completing_in_a_round_carries_the_wrapup_and_ends_the_rounds() {
    let home = Home::new();
    let models = script([
        Play::calls(&[("goal", r#"{"action":"complete"}"#)]),
        Play::Says("做完了：测试都绿了。"),
    ]);
    let handle = home.create_with(&models, &catalog()).await;
    let session = handle.id().clone();
    let change = GoalChange::Set {
        text: "把测试修绿".to_string(),
        limit: None,
    };
    ask(&handle, "g1", Command::Goal(change))
        .await
        .expect("会话在跑");
    let goal = until_goal(&handle, |goal| goal.state == GoalState::Completed).await;
    assert!(!goal.by_you, "人设的");
    until_ended(&home, &session, 1).await;
    let sent = rounds(&home, &session);
    assert!(
        sent[0].contains("Your standing objective, set by the user with `/goal`."),
        "{}",
        sent[0]
    );
    let results = results(&home, &session).await;
    assert!(
        results[0].0
            && results[0]
                .1
                .starts_with("<goal_complete>\nObjective: 把测试修绿\n"),
        "{results:?}"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(ended(&home, &session), 1, "完成以后不再开");
}

#[tokio::test]
async fn setting_another_is_refused_and_get_reads_the_current_one() {
    let home = Home::new();
    let models = script([
        Play::calls(&[
            ("goal", r#"{"action":"set","objective":"别的"}"#),
            ("goal", r#"{"action":"get"}"#),
        ]),
        Play::Says("好。"),
    ]);
    let handle = home.create_with(&models, &catalog()).await;
    let session = handle.id().clone();
    let change = GoalChange::Set {
        text: "把测试修绿".to_string(),
        limit: NonZeroU32::new(1),
    };
    ask(&handle, "g1", Command::Goal(change))
        .await
        .expect("会话在跑");
    // 第 1 轮调了工具，上限 1：到了上限记卡住。
    until_goal(&handle, |goal| goal.state == GoalState::Blocked).await;
    // 两件都只读、一起跑，结果先到先记（X-8 补：macOS 上 CI run 1882 两件倒过来到了）：排好再比。
    let mut results = results(&home, &session).await;
    results.sort();
    assert_eq!(
        results,
        [
            (
                false,
                "A goal is already set. Read it with action get. Only the user can change or clear it, with /goal."
                    .to_string()
            ),
            (
                true,
                "Objective: 把测试修绿\nState: running · Round 1 of 1".to_string()
            ),
        ]
    );
}

#[tokio::test]
async fn only_a_local_main_session_has_it() {
    let home = Home::new();
    let parent = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99").expect("合写法");
    let child = Lines {
        lineage: Some(Lineage {
            parent,
            depth: 1,
            workspace_from: None,
        }),
        ..Lines::default()
    };
    let group = Lines {
        venue: VenueId::parse("qq:group:123456").expect("合写法"),
        ..Lines::default()
    };
    for (lines, has) in [(Lines::default(), true), (child, false), (group, false)] {
        let models = script([Play::Says("好。")]);
        let handle = home
            .create_full(&models, &catalog(), Opening::default(), lines)
            .await;
        ask(&handle, "m1", say("你好")).await.expect("会话在跑");
        until_ended(&home, handle.id(), 1).await;
        let requests = models.requests();
        let names: Vec<&str> = requests[0]
            .1
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect();
        assert_eq!(names.contains(&"goal"), has, "{names:?}");
        assert!(names.contains(&"read"), "别的工具照给");
    }
}

/// 人插过话（施工 X-9，真跑看到的：人答了「照实报卡住」，她又凑了两轮）：续轮第 1 轮她问人、人答了，同一轮报卡住就收。
#[tokio::test]
async fn blocked_after_the_person_answered_skips_the_three_rounds() {
    use miyu_kernel::event::{Choice, Question, Response};
    use miyu_kernel::session::Answer;

    let home = Home::new();
    let question = Question {
        header: None,
        question: "密钥在哪？".to_string(),
        options: vec![
            Choice {
                label: "没有".to_string(),
                description: None,
                preview: None,
            },
            Choice {
                label: "我去找".to_string(),
                description: None,
                preview: None,
            },
        ],
        multiple: false,
    };
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut tools = miyu_goal::tools(&resources).expect("读得出");
    tools.push(Fake::new("ask", Access::Read, Act::Asks(vec![question])) as Arc<dyn Tool>);
    let catalog = Catalog::new(tools).expect("合写法");
    let models = script([
        Play::calls(&[("ask", "{}")]),
        Play::calls(&[("goal", r#"{"action":"blocked","reason":"没有密钥"}"#)]),
        Play::Says("卡在没有密钥。"),
    ]);
    let handle = home.create_with(&models, &catalog).await;
    let session = handle.id().clone();
    let change = GoalChange::Set {
        text: "拿真密钥请求 endpoint".to_string(),
        limit: None,
    };
    ask(&handle, "g1", Command::Goal(change))
        .await
        .expect("会话在跑");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let call_id = loop {
        let asked = home
            .log(&session)
            .into_iter()
            .find_map(|event| match event.body {
                Body::QuestionAsked(asked) => Some(asked.call_id),
                _ => None,
            });
        if let Some(call_id) = asked {
            break call_id;
        }
        assert!(tokio::time::Instant::now() < deadline, "六十秒内没问");
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    let answers = vec![Response {
        picked: vec!["没有".to_string()],
        text: None,
        notes: Some("照实报卡住就行".to_string()),
        attachments: Vec::new(),
    }];
    let answer = Command::Answer {
        call_id,
        answer: Answer::Questions(answers),
    };
    ask(&handle, "a1", answer).await.expect("会话在跑");
    let goal = until_goal(&handle, |goal| goal.state == GoalState::Blocked).await;
    assert_eq!(goal.round, 1, "第 1 轮就收了");
    let results = results(&home, &session).await;
    assert!(
        results
            .iter()
            .any(|(ok, text)| *ok && text.starts_with("<goal_blocked>")),
        "{results:?}"
    );
}

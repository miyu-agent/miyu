//! 长期目标的续轮（施工 X-7，`docs/blueprint/goal.md`）：真 actor、假模型。设了、空着就开续轮，第一轮完整版、之后短版；续轮里调了
//! 工具的接着开，没调工具的停下等人，人说一句以后接着开；到了上限记卡住；停了不开、接着跑重新发完整版；载入时还跑着的记成
//! 停着、不开；跑着的不闲够了退下。后台命令还在跑的先不开，它报完叫醒她、干完那一轮再开（施工 X-7 再补）。

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::block::Block;
use miyu_kernel::event::Body;
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, GoalChange};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Goal, GoalState, Handle, Retire};
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Exit, Tool};

use crate::support::*;

/// 有一件只读的假工具 `read` 的目录。
fn catalog() -> Catalog {
    let read = Fake::new("read", Access::Read, Act::Echo);
    Catalog::new([read as Arc<dyn Tool>]).expect("合写法")
}

/// 照 `plays` 回，起标题的请求回一个标题。
fn script(plays: impl IntoIterator<Item = Play>) -> Script {
    Script::new(plays).titles([Play::Says("测试")])
}

/// 改目标，alice 发的，等回应。
async fn goal(handle: &Handle, id: &str, change: GoalChange) {
    ask(handle, id, Command::Goal(change))
        .await
        .expect("会话在跑");
}

fn set(limit: Option<u32>) -> GoalChange {
    GoalChange::Set {
        text: "把测试修绿".to_string(),
        limit: limit.and_then(NonZeroU32::new),
    }
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

/// 日志里的续轮消息，照先后。
fn rounds(home: &Home, session: &SessionId) -> Vec<String> {
    home.log(session)
        .into_iter()
        .filter(|event| matches!(&event.by, By::Module(module) if module.id.as_str() == "goal"))
        .filter_map(|event| match event.body {
            Body::MessageUser(message) => {
                message.blocks.into_iter().find_map(|block| match block {
                    Block::Text(text) => Some(text.text),
                    _ => None,
                })
            }
            _ => None,
        })
        .collect()
}

/// 日志里结束了几轮。
fn ended(home: &Home, session: &SessionId) -> usize {
    home.log(session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .count()
}

#[tokio::test]
async fn rounds_run_while_idle_and_wait_after_one_without_tools() {
    let home = Home::new();
    let models = script([
        Play::calls(&[("read", "{}")]),
        Play::Says("读完了。"),
        Play::Says("在等你确认。"),
        Play::Says("好，接着来。"),
        Play::Says("还在等。"),
    ]);
    let handle = home.create_with(&models, &catalog()).await;
    let session = handle.id().clone();
    goal(&handle, "g1", set(Some(9))).await;
    let waiting = until_goal(&handle, |goal| goal.state == GoalState::Waiting).await;
    assert_eq!((waiting.round, ended(&home, &session)), (2, 2));
    let sent = rounds(&home, &session);
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert!(
        sent[0].contains("Your standing objective") && sent[0].contains("Round 1 of 9"),
        "第一轮完整版：{}",
        sent[0]
    );
    assert!(
        sent[0].contains(r#"Objective: "把测试修绿""#),
        "{}",
        sent[0]
    );
    assert!(
        sent[1].starts_with("<goal_round>\nRound 2 of 9 — your standing objective"),
        "之后短版：{}",
        sent[1]
    );
    assert_eq!(
        handle.retire(Duration::ZERO).await,
        Ok(Retire::Kept),
        "跑着的不闲够了退下"
    );
    // 人说一句：不等了，她回完以后接着开续轮，这一轮又没调工具，又等。
    ask(&handle, "m1", say("你继续")).await.expect("会话在跑");
    let again = until_goal(&handle, |goal| {
        goal.state == GoalState::Waiting && goal.round == 4
    })
    .await;
    assert_eq!(again.round, 4);
    assert_eq!(rounds(&home, &session).len(), 3);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(ended(&home, &session), 4, "等人的时候不再开");
}

#[tokio::test]
async fn reaching_the_limit_ends_the_goal_as_blocked() {
    let home = Home::new();
    let models = script([Play::calls(&[("read", "{}")]), Play::Says("读完了。")]);
    let handle = home.create_with(&models, &catalog()).await;
    goal(&handle, "g1", set(Some(1))).await;
    let blocked = until_goal(&handle, |goal| goal.state == GoalState::Blocked).await;
    assert!(
        blocked
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("round limit (1)")),
        "{blocked:?}"
    );
    assert_eq!(rounds(&home, handle.id()).len(), 1, "到了上限不再开");
}

#[tokio::test]
async fn pausing_stops_rounds_and_resuming_starts_over_with_the_full_message() {
    let home = Home::new();
    let models = script([Play::Says("在等你。"), Play::Says("还在等你。")]);
    let handle = home.create_with(&models, &catalog()).await;
    let session = handle.id().clone();
    goal(&handle, "g1", set(None)).await;
    until_goal(&handle, |goal| goal.state == GoalState::Waiting).await;
    goal(&handle, "g2", GoalChange::Pause).await;
    until_goal(&handle, |goal| goal.state == GoalState::Paused).await;
    goal(&handle, "g3", GoalChange::Resume).await;
    until_goal(&handle, |goal| {
        goal.state == GoalState::Waiting && goal.round == 2
    })
    .await;
    let sent = rounds(&home, &session);
    assert_eq!(sent.len(), 2);
    assert!(
        sent[1].contains("Your standing objective") && sent[1].contains("Round 2"),
        "接着跑的头一轮完整版：{}",
        sent[1]
    );
}

#[tokio::test]
async fn a_loaded_session_pauses_its_running_goal_until_resumed() {
    let home = Home::new();
    let models = script([Play::Says("在等你。"), Play::Says("接着了。")]);
    let handle = home.create_with(&models, &catalog()).await;
    let session = handle.id().clone();
    goal(&handle, "g1", set(None)).await;
    until_goal(&handle, |goal| goal.state == GoalState::Waiting).await;
    stop(&handle).await;
    let loaded = home.load_with(&session, &models, &catalog()).await;
    let paused = until_goal(&loaded, |goal| goal.state == GoalState::Paused).await;
    assert_eq!(paused.round, 1);
    let last = home.log(&session).pop().expect("有事件");
    assert!(
        matches!(last.body, Body::GoalPaused(_)) && last.by == By::Kernel,
        "载入时记成停着：{last:?}"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(ended(&home, &session), 1, "载入的不自己开");
    goal(&loaded, "g2", GoalChange::Resume).await;
    until_goal(&loaded, |goal| {
        goal.state == GoalState::Waiting && goal.round == 2
    })
    .await;
}

/// 头一直在要会话状态（施工 X-7 写测试时撞上的）：要状态的信排在收件箱里，续轮照样一轮轮开。原来只在送完一批时看一眼、收件箱
/// 不空就不开，办那封信又不再看，续轮就卡住了。
#[tokio::test]
async fn rounds_go_on_while_a_head_keeps_asking_for_the_status() {
    let home = Home::new();
    let models = script([
        Play::calls(&[("read", "{}")]),
        Play::Says("一。"),
        Play::calls(&[("read", "{}")]),
        Play::Says("二。"),
        Play::Says("在等你。"),
    ]);
    let handle = home.create_with(&models, &catalog()).await;
    let asking = tokio::spawn({
        let handle = handle.clone();
        async move {
            while handle.current().await.is_ok() {
                tokio::task::yield_now().await;
            }
        }
    });
    goal(&handle, "g1", set(None)).await;
    let waiting = until_goal(&handle, |goal| goal.state == GoalState::Waiting).await;
    assert_eq!(waiting.round, 3);
    asking.abort();
}

/// 有假工具 `read`、`start`（把 `held` 交给任务端口当后台命令）的目录（施工 X-7 再补）。
fn with_background(held: &Arc<Held>) -> Catalog {
    let read = Fake::new("read", Access::Read, Act::Echo);
    let start = Fake::new("start", Access::Read, Act::Background(Arc::clone(held)));
    Catalog::new([read as Arc<dyn Tool>, start as Arc<dyn Tool>]).expect("合写法")
}

/// 等到日志里结束了 `n` 轮：最多六十秒。
async fn until_ended(home: &Home, session: &SessionId, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while ended(home, session) < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "六十秒内没结束 {n} 轮"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_running_command_holds_the_round_until_it_reports() {
    let home = Home::new();
    let held = Held::new(&["building\n"]);
    let models = script([
        Play::calls(&[("start", "{}")]),
        Play::Says("开了编译，等它。"),
        Play::calls(&[("read", "{}")]),
        Play::Says("编译过了。"),
        Play::Says("做完了，等你看。"),
    ]);
    let handle = home.create_with(&models, &with_background(&held)).await;
    let session = handle.id().clone();
    goal(&handle, "g1", set(Some(9))).await;
    until_ended(&home, &session, 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        (ended(&home, &session), rounds(&home, &session).len()),
        (1, 1),
        "后台命令还在跑，续轮先不开"
    );
    let now = until_goal(&handle, |_| true).await;
    assert_eq!(now.state, GoalState::Running, "不是在等人");

    // 命令报完：叫醒她，那一轮干完再开续轮；续轮没调工具才等人。
    held.end(Exit::Code(0));
    let waiting = until_goal(&handle, |goal| goal.state == GoalState::Waiting).await;
    // 第几轮照跑着时开的每一轮算（`goal.rs`），叫醒的那一轮也算；续轮消息只发了两条。
    assert_eq!(
        (
            waiting.round,
            ended(&home, &session),
            rounds(&home, &session).len()
        ),
        (3, 3, 2),
        "叫醒的一轮、第二条续轮"
    );
    let log = home.log(&session);
    let reported = log
        .iter()
        .position(|event| matches!(event.body, Body::JobReported(_)))
        .expect("报完了");
    let second = log
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            matches!(&event.by, By::Module(module) if module.id.as_str() == "goal")
                && matches!(event.body, Body::MessageUser(_))
        })
        .nth(1)
        .map(|(at, _)| at)
        .expect("第二轮续轮");
    assert!(reported < second, "回报叫醒在前，续轮在后");
}

//! 长期目标给头看的（施工 X-8 补）：真 actor、假模型。目标的样子变了推一份（设、开一轮、等人开口）；续轮消息的命令编号照约定
//! 写着第几轮；只有续轮的会话拿目标原文当标题，不请求模型，人先说过话的照旧请求模型起。

use std::time::Duration;

use miyu_kernel::event::{Body, round_of};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, GoalChange};
use miyu_session::testkit::{Play, Script};
use miyu_session::{GoalState, Handle, Pushed};
use miyu_tool::Catalog;

use crate::support::*;

fn set(text: &str) -> Command {
    Command::Goal(GoalChange::Set {
        text: text.to_string(),
        limit: None,
    })
}

/// 等到日志里有一条起名的 `session.meta_changed`：最多六十秒，交回名字。
async fn until_titled(home: &Home, handle: &Handle) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let titled = home
            .log(handle.id())
            .into_iter()
            .find_map(|event| match event.body {
                Body::MetaChanged(changed) => changed.title,
                _ => None,
            });
        if let Some(title) = titled {
            return title;
        }
        assert!(tokio::time::Instant::now() < deadline, "六十秒内没起名");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn goal_changes_are_pushed_and_rounds_are_numbered() {
    let home = Home::new();
    let models = Script::new([Play::Says("在等你确认。")]).titles([Play::Says("测试")]);
    let handle = home.create_with(&models, &Catalog::default()).await;
    let mut subscription = handle.subscribe().await.expect("会话在跑");
    ask(&handle, "g1", set("把测试修绿"))
        .await
        .expect("会话在跑");
    // 设了、开第 1 轮、这一轮没调工具等人：照先后推，花的变了也推，去掉相邻重复的再比。
    let mut seen: Vec<(GoalState, u32)> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while seen.last() != Some(&(GoalState::Waiting, 1)) {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let pushed = tokio::time::timeout(left, subscription.next())
            .await
            .expect("六十秒内等到")
            .expect("订阅在");
        if let Pushed::Goal { goal, .. } = pushed.as_ref() {
            let goal = goal.as_ref().expect("有目标");
            let now = (goal.state, goal.round);
            if seen.last() != Some(&now) {
                seen.push(now);
            }
        }
    }
    assert_eq!(
        seen,
        [
            (GoalState::Running, 0),
            (GoalState::Running, 1),
            (GoalState::Waiting, 1)
        ]
    );
    let continuation = home
        .log(handle.id())
        .into_iter()
        .find(|event| matches!(&event.by, By::Module(module) if module.id.as_str() == "goal"))
        .expect("有续轮消息");
    assert_eq!(continuation.cause.as_ref().and_then(round_of), Some(1));
    // 只有续轮：照目标原文起名，没有起标题的请求（剧本里的「测试」没人要）。
    assert_eq!(until_titled(&home, &handle).await, "把测试修绿");
    assert_eq!(models.requests().len(), 1, "只有续轮那一次");
}

#[tokio::test]
async fn a_session_the_person_spoke_in_is_titled_as_usual() {
    let home = Home::new();
    // 头一次起标题回空的（没起成，算试过一次），再设目标、开第 1 轮：人说过话，照旧请求模型起。
    let models = Script::new([Play::Says("好。"), Play::Says("在等你确认。")])
        .titles([Play::Says(""), Play::Says("测试")]);
    let handle = home.create_with(&models, &Catalog::default()).await;
    ask(&handle, "m1", say("你好")).await.expect("会话在跑");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !home
        .log(handle.id())
        .iter()
        .any(|event| matches!(&event.body, Body::ModelCalled(called) if called.purpose.is_some()))
    {
        assert!(tokio::time::Instant::now() < deadline, "六十秒内没试起标题");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    ask(&handle, "g1", set("把测试修绿"))
        .await
        .expect("会话在跑");
    assert_eq!(until_titled(&home, &handle).await, "测试");
}

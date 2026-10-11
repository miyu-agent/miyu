//! 长期目标的样子随会话载入回来（施工 X-6，`docs/blueprint/goal.md`）：会话层照整份日志算，`Current` 交给头；之后每落一批
//! 加上这一批（`src/goal/tests.rs` 守一批批和从头算的一样）。

use miyu_kernel::event::Event;
use miyu_session::GoalState;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};

use crate::support::*;

#[tokio::test]
async fn a_loaded_session_knows_its_goal() {
    let home = Home::new();
    let models = Script::new([Play::Says("在。")]).titles([Play::Says("测试")]);
    let handle = home.create(&models).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "m1", say("在吗")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    drop(pushes);
    assert_eq!(
        handle.current().await.expect("在跑").goal,
        None,
        "没设过的没有"
    );
    let session = handle.id().clone();
    // 停下，日志空出来。
    stop(&handle).await;
    let dir = home.root.session_dir(&alice_account(), &session);
    let (mut log, _) = SessionLog::open(&dir, SEGMENT_LIMIT).expect("打得开");
    let next = log.next_seq().get();
    let line = |seq: u64, kind: &str, body: &str| {
        Event::from_line(&format!(
            r#"{{"seq":{seq},"at":"2026-10-12T01:00:00.000Z","kind":"{kind}","by":{{"kind":"person","account":"alice"}},"body":{body}}}"#
        ))
        .expect("读得懂")
    };
    log.append(&[
        line(next, "goal.set", r#"{"text":"修绿","limit":5}"#),
        line(next + 1, "goal.paused", "{}"),
    ])
    .expect("写得进");
    drop(log);

    let loaded = home.load(&session, &models).await;
    let goal = loaded
        .current()
        .await
        .expect("在跑")
        .goal
        .expect("载入的认得目标");
    assert_eq!(
        (goal.text.as_str(), goal.limit.map(|n| n.get()), goal.state),
        ("修绿", Some(5), GoalState::Paused)
    );
}

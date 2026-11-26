//! 定下要停以后不再接新连接（施工 CS-6，`internal/blueprint/core.md`「停下」第 5 条）：收尾那一段里头再连是「没在跑」，
//! 不会连上了却等不到回应；已经连着的照常说话；锁到收尾做完才放。三个平台一样，在进程里跑。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::support::{Head, Home, within};
use miyu_core::{Stopped, serve_then};
use miyu_ipc::{ConnectError, Lock, OpenError};
use miyu_session::testkit::Script;

#[tokio::test]
async fn a_stopping_core_refuses_new_heads_and_still_answers_its_own() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core(Arc::new(Script::new([])), &opened.token);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let (closed, close) = tokio::sync::oneshot::channel::<()>();
    let (closing, began) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::spawn(serve_then(
        opened.listener,
        core,
        || None,
        async move {
            let _stopped = stopped.await;
        },
        async move {
            let _began = closing.send(());
            let _closed = close.await;
        },
    ));
    let mut head = Head::connect(&home.root).await;
    let _stop = stop.send(());
    within("收尾开始", began).await.expect("开始了收尾");
    let reached = tokio::time::timeout(Duration::from_secs(5), miyu_ipc::connect(&home.root)).await;
    assert!(
        matches!(reached, Ok(Err(ConnectError::NotRunning))),
        "收尾时连是没在跑：{:?}",
        reached.map(|reached| reached.map(|_| ()))
    );
    let listed = head.call("s1", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "连着的照常说话：{listed}");
    assert!(
        matches!(Lock::acquire(&home.root), Err(OpenError::Running)),
        "收尾时锁还在"
    );
    let _closed = closed.send(());
    let stopped = within("停下", serving).await.expect("没 panic");
    assert_eq!(stopped, Stopped::Signal);
    Lock::acquire(&home.root).expect("交回以后锁放开了");
}

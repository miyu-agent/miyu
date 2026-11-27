//! 定下要停以后不再接新连接（施工 CS-6，`internal/blueprint/core.md`「停下」第 5 条）：收尾那一段里头再连是「没在跑」，
//! 不会连上了却等不到回应；已经连着的照常说话；锁到收尾做完才放。数据根里的库在放锁以前关上（施工 CS-9）。三个平台一样，
//! 在进程里跑。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::support::{Head, Home, within};
use miyu_core::{Stopped, serve_then};
use miyu_ipc::{ConnectError, Lock, OpenError};
use miyu_session::testkit::Script;
use miyu_store::sqlite::idle::open_under;

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

/// 放锁以前关上数据根里的库（施工 CS-9，修之前核心放下时才关、在放锁以后，还要把 `-wal` 合回主库）：另一个线程一拿到锁就数，
/// 一个开着的都没有；交回以后核心还在（测试手里还拿着它，真的核心也是放下运行时才放下它），库也照样关着。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_databases_are_closed_before_the_lock_goes() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core(Arc::new(Script::new([])), &opened.token);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::spawn(serve_then(
        opened.listener,
        Arc::clone(&core),
        || None,
        async move {
            let _stopped = stopped.await;
        },
        async {},
    ));
    let mut head = Head::connect(&home.root).await;
    let listed = head.call("s1", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "{listed}");
    drop(head);
    let path = home.root.path().to_path_buf();
    assert!(open_under(&path) > 0, "会话列表的索引、用量汇总开着");
    let (root, under) = (home.root.clone(), path.clone());
    let watching = std::thread::spawn(move || {
        loop {
            if let Ok(lock) = Lock::acquire(&root) {
                let open = open_under(&under);
                drop(lock);
                return open;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    let _stop = stop.send(());
    let stopped = within("停下", serving).await.expect("没 panic");
    assert_eq!(stopped, Stopped::Signal);
    let seen = tokio::task::spawn_blocking(move || watching.join())
        .await
        .expect("没 panic")
        .expect("没 panic");
    assert_eq!(seen, 0, "拿到锁的时候库都关上了");
    assert_eq!(open_under(&path), 0, "交回以后照样关着");
    drop(core);
}

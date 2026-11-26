//! 绑好了还不让头找到（施工 CS-6，`internal/blueprint/ipc.md`「核心起来」第 5 步）：`run/socket` 到 `publish` 才写，上一个留下的
//! 起来时先删，监听器丢了跟着删；锁可以先拿走，丢掉监听器不放锁。三个平台一样。

use miyu_ipc::{ConnectError, Lock, OpenError, connect, open_locked};

use crate::support::Home;

#[tokio::test]
async fn a_bound_core_is_found_only_once_it_publishes() {
    let home = Home::new();
    // 上一个核心崩了留下的位置：照它连不能连到还没开始接连接的这一个。
    std::fs::write(home.root.run().join("socket"), "/nowhere/core.sock\n").expect("写得进");
    let lock = Lock::acquire(&home.root).expect("拿得到锁");
    let mut opened = open_locked(&home.root, &home.dirs(), lock).expect("起得来");
    assert!(!home.root.run().join("socket").exists(), "留下的位置先删掉");
    assert!(
        matches!(connect(&home.root).await, Err(ConnectError::NotRunning)),
        "还没开始接连接的当没在跑"
    );
    opened.listener.publish().expect("写得进");
    assert_eq!(home.location(), opened.listener.path());
    let reached = connect(&home.root).await;
    assert!(reached.is_ok(), "写了就连得上");
    drop(reached);
    let lock = opened.listener.take_lock().expect("锁还在");
    assert!(opened.listener.take_lock().is_none(), "拿过的不再给");
    drop(opened);
    assert!(
        !home.root.run().join("socket").exists(),
        "丢了监听器删掉位置"
    );
    assert!(
        matches!(Lock::acquire(&home.root), Err(OpenError::Running)),
        "锁拿走了的，丢掉监听器不放锁"
    );
    drop(lock);
    Lock::acquire(&home.root).expect("锁放了");
}

//! 人停的核心不被头自动拉起（施工 CS-5，2026-11-17 项目主人定「停了就停，卡片带启动」）：`miyu core stop` 停了的，头断线以后
//! 自己重连（`reconnect_or_start`）不拉起、交回 `StoppedByUser`；人要用的（命令行的命令、头上的「启动」，`connect_or_start`）照拉，
//! 核心起来以后记号没了，以后崩了、空闲退出的照旧自动拉起。

use crate::support::{Home, count, within};
use miyu_ipc::{StartError, connect_or_start, reconnect_or_start};

#[tokio::test]
async fn a_core_stopped_by_a_person_is_started_again_only_when_asked() {
    let home = Home::new();
    let (connection, _) = within("拉起", connect_or_start(&home.root, || home.core_staying()))
        .await
        .expect("拉得起");
    drop(connection);
    let cwd = home.root.path().to_path_buf();
    let stopped = crate::support::cli::run(&cwd, &cwd, "zh_CN.UTF-8", &["core", "stop"]).await;
    assert_eq!(stopped.status.code(), Some(0), "{stopped:?}");
    let mark = home.root.run().join("stopped");
    assert!(mark.is_file(), "留下了人停的记号");

    // 头自己重连：不拉起。
    let held = within(
        "自己重连",
        reconnect_or_start(&home.root, || home.core_staying()),
    )
    .await;
    assert!(
        matches!(held, Err(StartError::StoppedByUser)),
        "{:?}",
        held.err()
    );
    assert_eq!(count(&home.core_log(), "starting"), 1, "没拉起");
    let said = StartError::StoppedByUser.said().expect("有说法");
    assert_eq!(said.key, "core/start/stopped_by_user");

    // 人要用（命令行的命令照这个连）：拉起来，记号没了。
    let (connection, _) = within("人要用", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    drop(connection);
    assert_eq!(count(&home.core_log(), "starting"), 2, "拉起了");
    assert!(!mark.exists(), "起来的核心删了记号");

    // 这一个空闲退出了：自己重连照旧拉起。
    home.until_stopped().await;
    let (connection, _) = within(
        "空闲退出以后自己重连",
        reconnect_or_start(&home.root, || home.core()),
    )
    .await
    .expect("拉得起");
    drop(connection);
    assert_eq!(
        count(&home.core_log(), "starting"),
        3,
        "{}",
        home.core_log()
    );
    home.until_stopped().await;
}

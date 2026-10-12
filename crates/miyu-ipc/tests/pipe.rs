//! Windows 上核心在命名管道上等连接，头连过去（施工 3-8 补）。

#![cfg(windows)]

use std::path::PathBuf;

use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

use crate::support::{Home, talk, within};
use miyu_ipc::{ConnectError, OpenError, connect, fingerprint, open};

/// 这个数据根的管道名。
fn pipe(home: &Home) -> PathBuf {
    PathBuf::from(format!(r"\\.\pipe\miyu-{}", fingerprint(&home.root)))
}

#[tokio::test]
async fn a_head_connects_through_the_pipe_and_gets_the_token() {
    let home = Home::new();
    let mut opened = open(&home.root, &home.dirs()).expect("起得来");
    assert_eq!(opened.listener.path(), pipe(&home));
    assert_eq!(home.location(), pipe(&home));
    assert_eq!(home.token(), opened.token);
    talk(&home, &mut opened).await;
    // 接走一个以后，下一个实例已经等着了。
    talk(&home, &mut opened).await;
}

#[tokio::test]
async fn a_head_that_leaves_at_once_does_not_jam_the_pipe() {
    let home = Home::new();
    let mut opened = open(&home.root, &home.dirs()).expect("起得来");
    // 连上就走，核心还没去接。
    drop(ClientOptions::new().open(pipe(&home)).expect("连得上"));
    // 核心去接：接到的是一个已经走了的连接（Windows 上是这样），报错也行；之后要照样接得了新的。
    match within("接", opened.listener.accept()).await {
        Ok(gone) => drop(gone),
        Err(error) => eprintln!("连上就走的那一个：{error}"),
    }
    talk(&home, &mut opened).await;
}

#[tokio::test]
async fn one_core_per_root_until_it_goes() {
    let home = Home::new();
    let first = open(&home.root, &home.dirs()).expect("起得来");
    let error = open(&home.root, &home.dirs()).expect_err("第二个起不来");
    assert!(matches!(error, OpenError::Running), "{error:?}");
    let old = first.token.clone();
    drop(first);
    let mut second = open(&home.root, &home.dirs()).expect("第一个走了以后起得来");
    assert_ne!(second.token, old, "每次起来换一个令牌");
    talk(&home, &mut second).await;
}

#[tokio::test]
async fn a_name_taken_by_another_program_is_left_alone() {
    let home = Home::new();
    let taken = ServerOptions::new()
        .first_pipe_instance(true)
        .create(pipe(&home))
        .expect("别的程序先占了这个名字");
    let error = open(&home.root, &home.dirs()).expect_err("名字被占了");
    assert!(
        matches!(&error, OpenError::Occupied(path) if *path == pipe(&home)),
        "{error:?}"
    );
    drop(taken);
    open(&home.root, &home.dirs()).expect("放开以后起得来");
}

#[tokio::test]
async fn without_a_core_there_is_nothing_to_connect_to() {
    let home = Home::new();
    let error = connect(&home.root).await.expect_err("没起过");
    assert!(matches!(error, ConnectError::NotRunning), "{error:?}");
    drop(open(&home.root, &home.dirs()).expect("起得来"));
    let error = connect(&home.root).await.expect_err("走了");
    assert!(matches!(error, ConnectError::NotRunning), "{error:?}");
}

/// 管道忙（第一个头占了等着的那个实例、核心还没去接，没开下一个）：再连的头歇着再试，核心过了 6 秒才接也照样连得上。
/// CI 的 Windows 机器负载重时核心慢，原来最多等 5 秒，`miyu ask` 报「All pipe instances are busy」（施工 ipc 补，
/// run 1938）。
#[tokio::test]
async fn a_busy_pipe_is_waited_out() {
    let home = Home::new();
    let mut opened = open(&home.root, &home.dirs()).expect("起得来");
    let first = ClientOptions::new().open(pipe(&home)).expect("连得上");
    let root = home.root.clone();
    let second = tokio::spawn(async move { connect(&root).await });
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    let taken = within("接到第一个", opened.listener.accept())
        .await
        .expect("接得到");
    let connected = within("第二个连上", second).await.expect("没 panic");
    assert!(connected.is_ok(), "{:?}", connected.err());
    drop((first, taken));
}

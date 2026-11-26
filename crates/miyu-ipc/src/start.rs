//! 头连核心，没在跑就拉起来（`internal/designs/12-进程形态与分发.md` 第二节「拉起时的握手」，施工 3-9 上）。
//!
//! 先连；连不上，拿 `run/spawn.lock` 这把拉起锁，免得几个头同时拉起好几个核心；拿到以后再连一次，
//! 别的头可能刚拉起来；还是连不上，拉起核心，把一根管道的写端给它当标准输出，等它写来的那一行，不轮询。
//! 真正保证一个数据根只跑一个核心的是核心自己拿的 `run/core.lock`（施工 3-8 下），拉起锁只是免得白拉。
//!
//! 拉起分两种（施工 CS-5）：人要用的（命令行的命令、头上的「启动」、头刚打开）照 [`connect_or_start`] 拉；头断线以后自己重连
//! 照 [`reconnect_or_start`]，人停的核心（`run/stopped`，[`crate::mark_stopped`]）不拉，交回 [`StartError::StoppedByUser`]。
//!
//! 等多久照 [`Patience`]（施工 CS-6）：过了 3 秒叫一声「慢了」（`_waiting` 的几个交给调用方的 [`Slow`]），最多 10 分钟；拉起的核心退了、
//! 说了起不来的当场结束。锁在别的核心手里、照 `run/socket` 又连不上的，是它正在起来（还没开始接连接）或者正在收尾：等它能连上，
//! 或者走了、放了锁再拉。

use std::fs::{File, OpenOptions, TryLockError};
use std::process::Command;

use miyu_store::root::DataRoot;

use crate::error::{ConnectError, StartError};
use crate::listener::Connection;
use crate::patience::{Clock, Patience, Slow};
use crate::ready::Ready;
use crate::{connect, connect_bare, lock};

mod spawn;
pub use spawn::{spawn_detached, spawn_detached_waiting};

/// 拉起锁的名字，在 `run/` 里。
const SPAWN_LOCK: &str = "spawn.lock";

/// 不叫「慢了」。
const QUIET: Slow<'static> = &|| {};

/// 连核心，没在跑就拉起来。数据根要已经建好骨架。`start` 给出启动核心的命令，例如主程序自己加上 `core`；
/// 标准输入输出、跟终端脱开，都由这里接。慢了不叫谁：要叫的照 [`connect_or_start_waiting`]。
///
/// 要在 tokio 运行时里调（要开着 IO 和时钟）。
///
/// # Errors
///
/// 核心起不来（它说了原因的、没说就退了的、等太久的）；别的头拉了太久；拉不起来；连不上。
pub async fn connect_or_start(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
) -> Result<(Connection, String), StartError> {
    connect_or_start_waiting(root, start, QUIET).await
}

/// 同 [`connect_or_start`]，慢了（过了 [`Patience::slow`] 还没连上）叫一次 `slow`（施工 CS-6）。
///
/// # Errors
///
/// 同 [`connect_or_start`]。
pub async fn connect_or_start_waiting(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
    slow: Slow<'_>,
) -> Result<(Connection, String), StartError> {
    let clock = Clock::new(Patience::current(), slow);
    start_with(root, Starting::Asked, start, connect, &clock).await
}

/// 同 [`connect_or_start`]，只是头断线以后自己重连用（施工 CS-5）：核心是人停的（`run/stopped`）就不拉，交回
/// [`StartError::StoppedByUser`]，头给人看「核心已停止」和「启动」，点了再照 [`connect_or_start`] 拉。崩了、空闲退出的照旧拉起。
///
/// # Errors
///
/// 同 [`connect_or_start`]，另有人停的 [`StartError::StoppedByUser`]。
pub async fn reconnect_or_start(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
) -> Result<(Connection, String), StartError> {
    reconnect_or_start_waiting(root, start, QUIET).await
}

/// 同 [`reconnect_or_start`]，慢了叫一次 `slow`（施工 CS-6）。
///
/// # Errors
///
/// 同 [`reconnect_or_start`]。
pub async fn reconnect_or_start_waiting(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
    slow: Slow<'_>,
) -> Result<(Connection, String), StartError> {
    let clock = Clock::new(Patience::current(), slow);
    start_with(root, Starting::Auto, start, connect, &clock).await
}

/// 同 [`connect_or_start`]，只是不读本机令牌（施工 W-8，[`connect_bare`]）：网页软件转发浏览器的连接用它。
///
/// # Errors
///
/// 同 [`connect_or_start`]。
pub async fn connect_or_start_bare(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
) -> Result<Connection, StartError> {
    connect_or_start_bare_waiting(root, start, QUIET).await
}

/// 同 [`connect_or_start_bare`]，慢了叫一次 `slow`（施工 CS-6）：网页软件照它给页面「核心还在启动」的状态。
///
/// # Errors
///
/// 同 [`connect_or_start`]。
pub async fn connect_or_start_bare_waiting(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
    slow: Slow<'_>,
) -> Result<Connection, StartError> {
    let clock = Clock::new(Patience::current(), slow);
    start_with(root, Starting::Asked, start, connect_bare, &clock).await
}

/// 同 [`reconnect_or_start`]，只是不读本机令牌（[`connect_bare`]）：网页软件断线以后替浏览器重连用。
///
/// # Errors
///
/// 同 [`reconnect_or_start`]。
pub async fn reconnect_or_start_bare(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
) -> Result<Connection, StartError> {
    reconnect_or_start_bare_waiting(root, start, QUIET).await
}

/// 同 [`reconnect_or_start_bare`]，慢了叫一次 `slow`（施工 CS-6）。
///
/// # Errors
///
/// 同 [`reconnect_or_start`]。
pub async fn reconnect_or_start_bare_waiting(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
    slow: Slow<'_>,
) -> Result<Connection, StartError> {
    let clock = Clock::new(Patience::current(), slow);
    start_with(root, Starting::Auto, start, connect_bare, &clock).await
}

/// 为什么拉起（施工 CS-5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Starting {
    /// 人要用：照拉，核心起来时自己删掉「人停的」记号。
    Asked,
    /// 头自己重连：人停的不拉。
    Auto,
}

/// 照 `connect` 连（读不读本机令牌由它定），没在跑就拉起来；`starting` 是自己重连、核心是人停的不拉。几段等共用 `clock`。
async fn start_with<'a, T, F, Fut>(
    root: &'a DataRoot,
    starting: Starting,
    start: impl FnOnce() -> Command,
    connect: F,
    clock: &Clock<'_>,
) -> Result<T, StartError>
where
    F: Fn(&'a DataRoot) -> Fut,
    Fut: Future<Output = Result<T, ConnectError>>,
{
    if let Some(connected) = connected(connect(root).await)? {
        return Ok(connected);
    }
    let held_back = || starting == Starting::Auto && crate::stopped::stopped_by_user(root);
    if held_back() {
        return Err(StartError::StoppedByUser);
    }
    let _spawning = match spawn_lock(root, &connect, clock).await? {
        Locked::Mine(file) => file,
        Locked::Reached(connected) => return Ok(connected),
    };
    if let Some(connected) = connected(connect(root).await)? {
        return Ok(connected);
    }
    // 锁在别的核心手里：它正在起来或者正在收尾，等它能连上、或者走了再拉（施工 CS-6）。
    if let Some(connected) = settle(root, &connect, clock).await? {
        return Ok(connected);
    }
    // 等拉起锁、等收尾的时候别的头可能刚停了它（人停的），拉以前再看一眼。
    if held_back() {
        return Err(StartError::StoppedByUser);
    }
    match spawn::launch(start(), root.path(), clock).await? {
        Ready::Ready => connect(root).await.map_err(StartError::Connect),
        // 拉起的那一个说已经有一个在跑：刚好有别的程序（服务管理器）抢先起了一个，等它能连上；它也走了的照没在跑说。
        Ready::Running => settle(root, &connect, clock)
            .await?
            .ok_or(StartError::Connect(ConnectError::NotRunning)),
        Ready::Failed { code, reason } => Err(StartError::Refused { code, reason }),
    }
}

/// 连上了的交回；核心没在跑的是 `None`；别的错照原样。
fn connected<T>(result: Result<T, ConnectError>) -> Result<Option<T>, StartError> {
    match result {
        Ok(connected) => Ok(Some(connected)),
        Err(ConnectError::NotRunning) => Ok(None),
        Err(error) => Err(StartError::Connect(error)),
    }
}

/// 拿拉起锁的结果。
enum Locked<T> {
    /// 拿到了。
    Mine(File),
    /// 等的时候别的头拉起来了，连上了。
    Reached(T),
}

/// 拿拉起锁：别的头在拉，等它拉完，每歇一下连一次，连上了就不要锁了。慢了叫一声，过了上限是 [`StartError::Busy`]。
async fn spawn_lock<'a, T, F, Fut>(
    root: &'a DataRoot,
    connect: &F,
    clock: &Clock<'_>,
) -> Result<Locked<T>, StartError>
where
    F: Fn(&'a DataRoot) -> Fut,
    Fut: Future<Output = Result<T, ConnectError>>,
{
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.run().join(SPAWN_LOCK))?;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Locked::Mine(file)),
            Err(TryLockError::WouldBlock) => {
                if let Some(connected) = connected(connect(root).await)? {
                    return Ok(Locked::Reached(connected));
                }
                if clock.expired() {
                    return Err(StartError::Busy);
                }
                clock.pause().await;
            }
            Err(TryLockError::Error(error)) => return Err(StartError::Io(error)),
        }
    }
}

/// 核心的锁被拿着、却连不上：它正在起来（套接字绑好了、还没开始接连接，`run/socket` 还没写）或者正在收尾（不再接连接了）。
/// 每歇一下连一次，连上了交回；锁放了（它走了）交回 `None`；慢了叫一声，过了上限是 [`StartError::Timeout`]。锁没人拿着的当场交回
/// `None`。
async fn settle<'a, T, F, Fut>(
    root: &'a DataRoot,
    connect: &F,
    clock: &Clock<'_>,
) -> Result<Option<T>, StartError>
where
    F: Fn(&'a DataRoot) -> Fut,
    Fut: Future<Output = Result<T, ConnectError>>,
{
    loop {
        if !lock::held(root)? {
            return Ok(None);
        }
        if let Some(connected) = connected(connect(root).await)? {
            return Ok(Some(connected));
        }
        if clock.expired() {
            return Err(StartError::Timeout);
        }
        clock.pause().await;
    }
}

#[cfg(test)]
mod tests;

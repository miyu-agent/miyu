//! 拉起一个程序、跟终端脱开、等它写来的那一行（施工 3-9 上；W-9 单独开放给网页软件拉起 `serve`；CS-6 改成照状态等）。
//!
//! 等的时候看三样：管道里来了一行（照 [`Ready`] 读）、管道到头了（什么都没写就退了，[`StartError::Silent`]）、拉起的进程退了。
//! 进程退了、管道却还没到头的（它起的子进程拿着管道的写端），再听一小会儿 [`GRACE`]：写了就退的那一行这时已经在管道里了；
//! 还是没有的照没说话就退了算，不等到上限。上限照 [`Clock`]：过了 3 秒叫一声「慢了」，最多 10 分钟。

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::error::StartError;
use crate::patience::{Clock, Patience, Slow};
use crate::ready::Ready;
use crate::{heard, sys};

/// 拉起的进程退了以后，再听多久管道（见这一页开头）。
const GRACE: Duration = Duration::from_secs(1);

/// 拉起 `command`、跟终端脱开，工作目录是 `dir`，等它往标准输出写来的那一行（[`Ready`]），最多 10 分钟（施工 CS-6，原来 60 秒），
/// 它退了的当场结束。核心照它拉起，网页软件的 `open` 也照它拉起 `serve`（施工 W-9，`web-module.md`「怎么走」第十一条第 2 款）。
/// 慢了不叫谁：要叫的照 [`spawn_detached_waiting`]。
///
/// # Errors
///
/// 拉不起来；什么都没写就退了；等太久。
pub async fn spawn_detached(command: Command, dir: &Path) -> Result<Ready, StartError> {
    spawn_detached_waiting(command, dir, &|| {}).await
}

/// 同 [`spawn_detached`]，慢了（过了 [`Patience::slow`] 还没写来那一行）叫一次 `slow`（施工 CS-6）。
///
/// # Errors
///
/// 同 [`spawn_detached`]。
pub async fn spawn_detached_waiting(
    command: Command,
    dir: &Path,
    slow: Slow<'_>,
) -> Result<Ready, StartError> {
    launch(command, dir, &Clock::new(Patience::current(), slow)).await
}

/// 拉起核心（或者别的程序），等它写来的那一行，照 `clock` 等。拉起来的跟终端脱开：标准输入、标准错误接空，标准输出是那根管道；
/// 工作目录是 `dir`（核心的是数据根，不占着头的当前目录：施工 3-9 下的真机验收里查出这一条原先没做，核心一直占着敲
/// `miyu ask` 时所在的目录，那是一块移动硬盘的话就卸不下来）。头不等它退出，另起一个线程替它收尸，收了告诉这边。
pub(crate) async fn launch(
    mut command: Command,
    dir: &Path,
    clock: &Clock<'_>,
) -> Result<Ready, StartError> {
    let (reader, writer) = io::pipe()?;
    command
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::null());
    sys::detach(&mut command);
    let mut child = command.spawn()?;
    // 命令里还攥着写端的这一份：不放下，核心没说话就退了，这边也读不到头。
    drop(command);
    let (exited, gone) = oneshot::channel();
    std::thread::spawn(move || match child.wait() {
        // 这边不再听了的，交不出去也没事。
        Ok(_) => drop(exited.send(())),
        Err(error) => tracing::debug!(target: "miyu::ipc", error = %error, "core not reaped"),
    });
    hear(heard::first_line(reader), gone, clock).await
}

/// 等那一行：`line` 读第一行，`gone` 是拉起的进程退了（收尸的线程没收成的不来）。见这一页开头。
pub(crate) async fn hear(
    line: impl Future<Output = io::Result<Option<String>>>,
    gone: oneshot::Receiver<()>,
    clock: &Clock<'_>,
) -> Result<Ready, StartError> {
    tokio::pin!(line, gone);
    // 进程退了以后最多听到这一刻；没退的听到上限。
    let mut until = clock.deadline();
    let mut exited = false;
    loop {
        tokio::select! {
            read = &mut line => return match read {
                Ok(Some(line)) => Ok(Ready::parse(&line)),
                Ok(None) => Err(StartError::Silent),
                Err(error) => Err(StartError::Io(error)),
            },
            waited = &mut gone, if !exited => {
                exited = true;
                if waited.is_ok() {
                    until = until.min(Instant::now() + GRACE);
                }
            }
            () = clock.tell() => {}
            () = tokio::time::sleep_until(until) => {
                return Err(match until < clock.deadline() {
                    true => StartError::Silent,
                    false => StartError::Timeout,
                });
            }
        }
    }
}

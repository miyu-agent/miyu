//! 读拉起的程序往管道里写的第一行（施工 CS-6）：读的这一段随时能放下，放下了不留下堵着的线程。
//!
//! 原来在阻塞线程池里读（`spawn_blocking`），外面套一个时限：时限到了，读的线程还堵在管道上，头放下运行时要等它，于是 60 秒到了
//! 也要等核心写出那一行才印得出报错、退得了（2026-11-27 量到 138 秒）；网页这种一直开着的，每超时一次漏一个线程。
//!
//! - Unix：管道的读端交给 tokio 当异步的读（`pipe::Receiver`），放下就关，什么都不留。
//! - Windows：匿名管道不能异步地读，另起一条不归运行时管的线程读，读到了经通道交回来。放下了只是不再听，那条线程读到那一行、或者
//!   拉起的程序退了（管道到头）就自己走；不在线程池里，放下运行时、进程退出都不等它。

use std::io::{self, PipeReader};

/// 那一行最长多少字节：原因写得再长也只读这么多。
const LINE_LIMIT: u64 = 4096;

/// 读第一行：读到头（什么都没写就关了）的是 `None`。
#[cfg(unix)]
pub(crate) async fn first_line(reader: PipeReader) -> io::Result<Option<String>> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

    let receiver =
        tokio::net::unix::pipe::Receiver::from_owned_fd(std::os::fd::OwnedFd::from(reader))?;
    let mut line = String::new();
    let read = BufReader::new(receiver.take(LINE_LIMIT))
        .read_line(&mut line)
        .await?;
    Ok((read > 0).then_some(line))
}

/// 读第一行：读到头（什么都没写就关了）的是 `None`。
#[cfg(windows)]
pub(crate) async fn first_line(reader: PipeReader) -> io::Result<Option<String>> {
    let (sender, heard) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("miyu-ready-line".to_string())
        .spawn(move || {
            // 这边不再听了（等太久、放下了）的，交不出去也没事。
            drop(sender.send(blocking_line(reader)));
        })?;
    heard
        .await
        .unwrap_or_else(|_| Err(io::Error::other("ready line reader gone")))
}

/// 阻塞地读第一行。
#[cfg(windows)]
fn blocking_line(reader: PipeReader) -> io::Result<Option<String>> {
    use std::io::{BufRead, BufReader, Read};

    let mut line = String::new();
    let read = BufReader::new(reader.take(LINE_LIMIT)).read_line(&mut line)?;
    Ok((read > 0).then_some(line))
}

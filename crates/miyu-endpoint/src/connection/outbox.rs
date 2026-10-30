//! 写的一头（`04-核心协议.md` 第七节）：写队列里的一行行照先后写出去，一行一个 `\n`。
//!
//! 已经排着的几行一起写、只冲一次（施工 V-22）：流式时一个字一条推送，以前每条两次写、一次冲，一轮几百条就是上千次系统
//! 调用。只收这时已经在队列里的，不等后面的：不多一点延迟。几行合在一起写，字节、先后和一行行写的一模一样：回应、推送都从
//! 这一个队列来，先后本来就是入队的先后。

use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

/// 一次最多攒多少字节再写：再多的留到下一次，一次写不出几 MB。
const BATCH: usize = 256 * 1024;

/// 攒的缓冲写完以后留多大：攒过一次大的（一页历史）不一直占着。
const KEEP: usize = 64 * 1024;

/// 写的一头：一行行写出去，写不出去就停。
pub(super) async fn write_all<W: AsyncWrite + Unpin>(
    mut write: W,
    mut lines: mpsc::Receiver<String>,
) {
    let mut batch = Vec::new();
    while let Some(line) = lines.recv().await {
        push(&mut batch, &line);
        while batch.len() < BATCH {
            match lines.try_recv() {
                Ok(line) => push(&mut batch, &line),
                Err(_) => break,
            }
        }
        let written = async {
            write.write_all(&batch).await?;
            write.flush().await
        };
        if written.await.is_err() {
            return;
        }
        batch.clear();
        batch.shrink_to(KEEP);
    }
}

/// 一行接上它的 `\n`。
fn push(batch: &mut Vec<u8>, line: &str) {
    batch.extend_from_slice(line.as_bytes());
    batch.push(b'\n');
}

#[cfg(test)]
mod tests;

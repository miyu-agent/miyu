//! 请求体里有 blob 的那一截攒成大块交（施工 PF-1，`docs/blueprint/http.md`「怎么走」第 3 条「边编边发」）。
//!
//! 为什么：原来一张图一个阻塞任务、48 KiB 一块经通道交过来，交一块、等一块。长会话的请求体里夹着一二十张工具读回来的小图，
//! 2 MB 的请求体要交七八十块，每一块都要阻塞线程和运行时的线程互相叫醒一次。不压 CPU 时叫醒几乎不花时间（1.77 MB 4 ms）；
//! CPU 被抢时每叫醒一次都要重新排队，压测（bm「组合加码」，2026-11-13）里收齐一个请求体从 4 ms 涨到 0.3～3 秒。
//!
//! 怎么做：碰到第一段 blob，起一个阻塞任务把从这一段起剩下的全部生成出来（字照拷、blob 照读照编），攒满 [`BATCH`] 交一块，
//! 最多压一块；比 [`BATCH`] 还大的一段字不拷，先交攒着的，再照原样交它。块的边界不影响字节：拼起来和整份编的一个字节都不差，
//! 哈希照旧在交出去时算（`body.rs` 的 `Stream::give`）。

use std::io;
use std::mem;

use bytes::Bytes;
use tokio::sync::mpsc;

use super::{Piece, Replay};

/// 攒多少个字节交一块：1 MiB。2 MB 上下的请求体交两三块；一张大图的原图和 base64 照旧不整份出现在内存里，手里最多是
/// 通道里压着的一块、正在攒的一块，加上 HTTP 客户端正在写的一块。
pub(super) const BATCH: usize = 1 << 20;

/// 从第 `from` 段起把剩下的全部生成出来，攒满 [`BATCH`] 经 `sender` 交一块；读不了 blob 的交一个错就停，那边不收了也停。
/// 在阻塞线程里调。
pub(super) fn produce(replay: &Replay, from: usize, sender: &mpsc::Sender<io::Result<Bytes>>) {
    let mut out = Batches {
        sender,
        buffer: Vec::new(),
        closed: false,
    };
    for piece in replay.body.pieces.iter().skip(from) {
        match piece {
            Piece::Text(text) if text.len() >= BATCH => {
                out.flush();
                out.give(text.clone());
            }
            Piece::Text(text) => out.push(text),
            // 空的不打开（和整份编的一样，什么都不写）。
            Piece::Base64 { size: 0, .. } => {}
            Piece::Base64 { blob, size } => {
                let read = replay.read_blob(blob, *size, 0, |_, chunk| {
                    out.push(&chunk);
                    !out.closed
                });
                if let Err(error) = read {
                    // 那边已经不收了的不用再说。
                    drop(sender.blocking_send(Err(error)));
                    return;
                }
            }
        }
        if out.closed {
            return;
        }
    }
    out.flush();
}

/// 攒着的一块，和交到哪。
struct Batches<'a> {
    sender: &'a mpsc::Sender<io::Result<Bytes>>,
    buffer: Vec<u8>,
    /// 那边不收了：不再攒、不再交。
    closed: bool,
}

impl Batches<'_> {
    /// 接上这些字节，攒满正好 [`BATCH`] 交一块，余下的接着攒：块不超过 [`BATCH`]，攒的时候也不用一倍一倍地搬。
    fn push(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() && !self.closed {
            if self.buffer.capacity() == 0 {
                self.buffer.reserve_exact(BATCH);
            }
            let room = BATCH.saturating_sub(self.buffer.len()).min(bytes.len());
            let (now, later) = bytes.split_at(room);
            self.buffer.extend_from_slice(now);
            bytes = later;
            if self.buffer.len() >= BATCH {
                self.flush();
            }
        }
    }

    /// 攒着的交出去（没有攒着的不交）；下一块用到时再留地方。
    fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let full = mem::take(&mut self.buffer);
        self.give(Bytes::from(full));
    }

    /// 交一块；那边收得慢就等着（通道只压一块）。
    fn give(&mut self, chunk: Bytes) {
        if !self.closed && self.sender.blocking_send(Ok(chunk)).is_err() {
            self.closed = true;
        }
    }
}

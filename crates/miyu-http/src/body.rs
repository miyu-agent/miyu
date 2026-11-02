//! 请求体边编边发（施工 V-38）：驱动编好的片段清单（[`Fragments`]）照先后一块一块地送出去。编好的字照原样送，多处共享不拷；
//! blob 那几段在阻塞线程里一小块一小块地读（[`base64::CHUNK`]，48 KiB）、编成 base64 送出去，送完就扔，一张图的原图和
//! base64 都不在内存里整份出现。
//!
//! - 重放：片段清单只有字和 blob 的引用，同一份可以从头再造一条流（跳转、对方没处理的重发，`resend.rs`），字节一样。
//! - 哈希：请求体的 SHA-256 跟着送出去的字节一路算，几条流共用一份：哪一条先送到哪，就算到哪，重放的那一条从算过的地方
//!   接着算。对方没等送完就回了响应头（401、413 这类），剩下的照样生成出来算完（[`Replay::hash`]），只是不往外送。
//! - 长短：`Content-Length` 照片段清单算，和以前整份的字节一样长；blob 读出来的比编码时量的短了、长了都算出错，请求断掉。

use std::io::{self, Read};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::{Frame, SizeHint};
use miyu_drivers::{Fragment, Fragments, base64};
use miyu_kernel::id::{ContentHash, Hasher};
use tokio::sync::mpsc;

/// 发请求体时从哪里读 blob（施工 V-38）：核心照数据根里的 blob 读，核对的规矩照存储那边的。
pub trait BlobSource: Send + Sync {
    /// 打开 `blob`，从第 `offset` 个字节读起。在阻塞线程里调。
    ///
    /// # Errors
    ///
    /// 没有这个 blob、读出来和名字对不上、读不了。
    fn open(&self, blob: &ContentHash, offset: u64) -> io::Result<Box<dyn Read + Send>>;
}

/// 一次请求的请求体：驱动编好的片段清单，和 blob 那几段从哪里读（施工 V-38）。
pub struct Body {
    pieces: Vec<Piece>,
    len: u64,
    blobs: Option<Arc<dyn BlobSource>>,
}

/// 请求体里的一段：编好的字共享着送，blob 照大小算长短、送的时候读。
enum Piece {
    Text(Bytes),
    Base64 { blob: ContentHash, size: u64 },
}

impl Piece {
    fn len(&self) -> u64 {
        match self {
            Piece::Text(text) => u64::try_from(text.len()).unwrap_or(u64::MAX),
            Piece::Base64 { size, .. } => base64::encoded_len(*size),
        }
    }
}

impl Body {
    /// 照片段清单发，blob 那几段从 `blobs` 读。
    pub fn new(fragments: Fragments, blobs: Arc<dyn BlobSource>) -> Body {
        let mut body = Body::from(fragments);
        body.blobs = Some(blobs);
        body
    }

    /// 一共多少个字节。
    pub fn len(&self) -> u64 {
        self.len
    }

    /// 一个字节都没有。
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// 没有 blob 的片段清单（试一家这类）：有 blob 的那几段送的时候出错。
impl From<Fragments> for Body {
    fn from(fragments: Fragments) -> Body {
        let len = fragments.len();
        let pieces = fragments
            .into_parts()
            .into_iter()
            .map(|part| match part {
                // 交出去不拷：字的那一块直接成了共享的 `Bytes`。
                Fragment::Text(text) => Piece::Text(Bytes::from(text)),
                Fragment::Base64 { blob, size } => Piece::Base64 { blob, size },
            })
            .collect();
        Body {
            pieces,
            len,
            blobs: None,
        }
    }
}

/// 整份的字节（测试用）。
impl From<Vec<u8>> for Body {
    fn from(bytes: Vec<u8>) -> Body {
        Body::from(Fragments::from(bytes))
    }
}

/// 发的那一次手里的请求体：能一遍遍从头造流，哈希几条流共用一份（见模块的说明）。
pub(crate) struct Replay {
    body: Body,
    hash: Mutex<Hashing>,
}

/// 哈希算到哪了。
struct Hashing {
    /// 还在算的；算完了的没有。
    hasher: Option<Hasher>,
    /// 前多少个字节算过了。
    done: u64,
    /// 算完的结果。
    result: Option<ContentHash>,
}

impl Replay {
    pub(crate) fn new(body: Body) -> Replay {
        Replay {
            body,
            hash: Mutex::new(Hashing {
                hasher: Some(Hasher::default()),
                done: 0,
                result: None,
            }),
        }
    }

    /// 从头造一条流。
    pub(crate) fn stream(self: &Arc<Self>) -> Stream {
        Stream {
            replay: Arc::clone(self),
            next: 0,
            at: 0,
            piece_from: 0,
            blob: None,
        }
    }

    /// 送到第 `at` 个字节的这一块：没算过的那一截接着算；送完了收尾。
    fn feed(&self, at: u64, bytes: &[u8]) {
        let mut hashing = self.hash.lock().unwrap_or_else(PoisonError::into_inner);
        let end = at.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if end <= hashing.done || at > hashing.done {
            return;
        }
        let from = usize::try_from(hashing.done - at).unwrap_or(usize::MAX);
        if let Some(hasher) = hashing.hasher.as_mut()
            && let Some(rest) = bytes.get(from..)
        {
            hasher.update(rest);
            hashing.done = end;
        }
        if hashing.done >= self.body.len {
            settle(&mut hashing);
        }
    }

    /// 请求体的 SHA-256：没送完的（响应头早到了、被叫停了、造不出请求的）把剩下的生成出来算完，不往外送；要读 blob 的在阻塞
    /// 线程里。剩下的读不出来的，照生成出来的那些算（请求本来也已经出错了）。
    pub(crate) async fn hash(self: &Arc<Self>) -> ContentHash {
        let done = {
            let mut hashing = self.hash.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(result) = &hashing.result {
                return result.clone();
            }
            if hashing.done >= self.body.len {
                return settle(&mut hashing);
            }
            hashing.done
        };
        if !self.blob_after(done) {
            return self.complete();
        }
        let replay = Arc::clone(self);
        match tokio::task::spawn_blocking(move || replay.complete()).await {
            Ok(hash) => hash,
            // 阻塞线程里崩了：照算到的收尾。
            Err(_) => settle(&mut self.hash.lock().unwrap_or_else(PoisonError::into_inner)),
        }
    }

    /// 第 `done` 个字节以后还有没有 blob 那一段。
    fn blob_after(&self, done: u64) -> bool {
        let mut at = 0u64;
        self.body.pieces.iter().any(|piece| {
            let end = at.saturating_add(piece.len());
            let blob = matches!(piece, Piece::Base64 { .. }) && end > done;
            at = end;
            blob
        })
    }

    /// 从算到的地方生成到最后，算完交回。
    fn complete(&self) -> ContentHash {
        let mut at = 0u64;
        for piece in &self.body.pieces {
            let len = piece.len();
            let done = self.done();
            if at.saturating_add(len) > done {
                match piece {
                    Piece::Text(text) => self.feed(at, text),
                    Piece::Base64 { blob, size } => {
                        let start = at;
                        let skip = done.saturating_sub(at);
                        let produced = self.read_blob(blob, *size, skip, |offset, chunk| {
                            self.feed(start + offset, &chunk);
                            true
                        });
                        if produced.is_err() {
                            break;
                        }
                    }
                }
            }
            at = at.saturating_add(len);
        }
        settle(&mut self.hash.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn done(&self) -> u64 {
        self.hash
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .done
    }

    /// 读一个 blob 编成 base64，从编好的第 `skip` 个字节起一块一块交给 `each`（交回假的就停）：`each` 收到的是这一块在这一段
    /// 里从哪起、这一块。在阻塞线程里调。
    fn read_blob(
        &self,
        blob: &ContentHash,
        size: u64,
        skip: u64,
        mut each: impl FnMut(u64, Bytes) -> bool,
    ) -> io::Result<()> {
        let source = self
            .body
            .blobs
            .as_ref()
            .ok_or_else(|| io::Error::other("请求体里有 blob，却没有读 blob 的地方"))?;
        // 从 `skip` 所在的那一组（4 个字对 3 个字节）读起：块的开头都在组的边上，编出来和整份编的一样。
        let mut offset = skip / 4 * 4;
        let start = skip / 4 * 3;
        let mut left = size.saturating_sub(start);
        let mut reader = source.open(blob, start)?;
        let mut raw = vec![0u8; base64::CHUNK];
        while left > 0 {
            let want = usize::try_from(left).map_or(raw.len(), |left| left.min(raw.len()));
            reader.read_exact(&mut raw[..want]).map_err(|error| {
                if error.kind() == io::ErrorKind::UnexpectedEof {
                    io::Error::other(format!("blob {blob} 比编码时量的短"))
                } else {
                    error
                }
            })?;
            left -= u64::try_from(want).unwrap_or(u64::MAX);
            let chunk = Bytes::from(base64::encode(&raw[..want]).into_bytes());
            let length = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
            // 这一块里算过、送过的那一截不再交。
            let cut = skip.saturating_sub(offset).min(length);
            if cut < length {
                let from = usize::try_from(cut).unwrap_or(chunk.len());
                if !each(offset + cut, chunk.slice(from..)) {
                    return Ok(());
                }
            }
            offset += length;
        }
        let mut more = [0u8; 1];
        loop {
            match reader.read(&mut more) {
                Ok(0) => return Ok(()),
                Ok(_) => return Err(io::Error::other(format!("blob {blob} 比编码时量的长"))),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }
}

/// 算完：交回结果，记下。
fn settle(hashing: &mut Hashing) -> ContentHash {
    if let Some(result) = &hashing.result {
        return result.clone();
    }
    let result = hashing.hasher.take().unwrap_or_default().finish();
    hashing.result = Some(result.clone());
    result
}

/// 一条流：照片段清单从头一块一块地交给 HTTP 客户端。
pub(crate) struct Stream {
    replay: Arc<Replay>,
    /// 下一段是第几段。
    next: usize,
    /// 交出去了多少个字节。
    at: u64,
    /// 正在读的那一段从第几个字节起。
    piece_from: u64,
    /// 正在读的 blob：阻塞线程编好一块交过来一块。
    blob: Option<mpsc::Receiver<io::Result<Bytes>>>,
}

impl Stream {
    /// 交出这一块：算进哈希，往后挪。
    fn give(&mut self, chunk: Bytes) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        self.replay.feed(self.at, &chunk);
        self.at = self
            .at
            .saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        Poll::Ready(Some(Ok(Frame::data(chunk))))
    }

    /// 出错了：这条流不再往下走。
    fn fail(&mut self, error: io::Error) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        self.blob = None;
        self.next = self.replay.body.pieces.len();
        Poll::Ready(Some(Err(error)))
    }

    /// 在阻塞线程里读第 `next` 段的 blob，编好一块交过来一块；这边收得慢，那边就等着（最多压一块）。
    fn start(
        &mut self,
        blob: &ContentHash,
        size: u64,
    ) -> io::Result<mpsc::Receiver<io::Result<Bytes>>> {
        let runtime = tokio::runtime::Handle::try_current().map_err(io::Error::other)?;
        let (sender, receiver) = mpsc::channel(1);
        let replay = Arc::clone(&self.replay);
        let blob = blob.clone();
        runtime.spawn_blocking(move || {
            let read = replay.read_blob(&blob, size, 0, |_, chunk| {
                sender.blocking_send(Ok(chunk)).is_ok()
            });
            if let Err(error) = read {
                // 这边已经不收了的不用再说。
                drop(sender.blocking_send(Err(error)));
            }
        });
        Ok(receiver)
    }
}

impl http_body::Body for Stream {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        let this = &mut *self;
        loop {
            if let Some(receiver) = this.blob.as_mut() {
                match receiver.poll_recv(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Some(Ok(chunk))) => return this.give(chunk),
                    Poll::Ready(Some(Err(error))) => return this.fail(error),
                    Poll::Ready(None) => {
                        this.blob = None;
                        let expected = this.replay.body.pieces.get(this.next).map_or(0, Piece::len);
                        if this.at - this.piece_from != expected {
                            return this.fail(io::Error::other("读 blob 的线程没读完就停了"));
                        }
                        this.next += 1;
                        continue;
                    }
                }
            }
            let replay = Arc::clone(&this.replay);
            let Some(piece) = replay.body.pieces.get(this.next) else {
                return Poll::Ready(None);
            };
            this.piece_from = this.at;
            match piece {
                Piece::Text(text) => {
                    this.next += 1;
                    if !text.is_empty() {
                        return this.give(text.clone());
                    }
                }
                Piece::Base64 { size: 0, .. } => this.next += 1,
                Piece::Base64 { blob, size } => match this.start(blob, *size) {
                    Ok(receiver) => this.blob = Some(receiver),
                    Err(error) => return this.fail(error),
                },
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.at >= self.replay.body.len
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.replay.body.len.saturating_sub(self.at))
    }
}

#[cfg(test)]
mod tests;

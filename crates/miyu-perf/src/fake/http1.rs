//! HTTP/1.1 的传输：明文的口和 HTTPS 只说 `http/1.1` 的口共用（施工 LW-1 从 `fake.rs` 挪出来）。一个连接读一个请求，回完关上。

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::shared::{Reply, Request, Shared};
use super::{Head, Transport, parse_head};

/// 一个连接：读一个请求，交给 [`Shared::arrive`]，照它定的回应写完关上，写完的时刻交给量尺。`via` 是这个连接从哪个口来。
pub(super) async fn serve<S>(mut socket: S, shared: Arc<Shared>, via: Transport)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Some((head, body, at)) = read_request(&mut socket).await else {
        return;
    };
    let request = Request {
        post: head.method == "POST",
        body,
        at,
        via,
    };
    let Some(reply) = shared.arrive(request) else {
        return;
    };
    if write(&mut socket, &reply).await {
        reply.written();
        #[expect(clippy::let_underscore_must_use, reason = "回完了，关不干净也不影响量")]
        let _ = socket.shutdown().await;
    }
}

/// 读一个请求：头读全记下时刻，再照 `Content-Length` 读完请求体。连接断了、头读不懂的是 `None`。
async fn read_request<S>(socket: &mut S) -> Option<(Head, Vec<u8>, std::time::Instant)>
where
    S: AsyncRead + Unpin,
{
    let mut data = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let (head, at) = loop {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..read]);
        if let Some(head) = parse_head(&data) {
            break (head, std::time::Instant::now());
        }
    };
    let mut body = data.split_off(head.size);
    while body.len() < head.length {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Some((head, body, at))
}

/// 写回应。事件流不写长度、`Connection: close`，写完关上算结尾；模型列表写长度。不匀着的头和正文一次写完；匀着的（施工
/// MD-4）头先写，之后一个事件一写、隔开 [`Reply::pace`]。写不进的是 `false`。
async fn write<S>(socket: &mut S, reply: &Reply) -> bool
where
    S: AsyncWrite + Unpin,
{
    let length = match reply.stream {
        true => String::new(),
        false => format!("Content-Length: {}\r\n", reply.body.len()),
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\n{length}Connection: close\r\n\r\n",
        reply.content_type()
    );
    let Some(gap) = reply.pace else {
        let response = [head.as_bytes(), reply.body.as_bytes()].concat();
        return socket.write_all(&response).await.is_ok();
    };
    if socket.write_all(head.as_bytes()).await.is_err() {
        return false;
    }
    for (n, piece) in reply.pieces().into_iter().enumerate() {
        if n > 0 {
            tokio::time::sleep(gap).await;
        }
        if socket.write_all(piece.as_bytes()).await.is_err() {
            return false;
        }
    }
    true
}

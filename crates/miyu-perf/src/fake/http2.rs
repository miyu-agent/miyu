//! HTTP/2 的传输（施工 LW-1）：HTTPS 只说 `h2` 的口。一条连接上一个流一个请求，每个流各自读完请求体、照 [`Shared::arrive`]
//! 定的回应写完；连接由核心那边留着复用，和真供应商一样。

use std::sync::Arc;

use bytes::Bytes;
use h2::RecvStream;
use h2::server::SendResponse;
use tokio::io::{AsyncRead, AsyncWrite};

use super::Transport;
use super::shared::{Request, Shared};

/// 一条连接：握手（HTTP/2 的前言和设置），之后每来一个流交给一个任务。连接坏了、对方关了就走。
pub(super) async fn serve<S>(socket: S, shared: Arc<Shared>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let Ok(mut connection) = h2::server::handshake(socket).await else {
        return;
    };
    // `accept` 同时推着整条连接收发：要一直调到它交回 `None`。
    while let Some(Ok((request, respond))) = connection.accept().await {
        tokio::spawn(stream(request, respond, Arc::clone(&shared)));
    }
}

/// 一个流：头到了记下时刻，读完请求体（读多少还对方多少窗口），回 200 和定好的回应，写完交给量尺。
async fn stream(
    request: http::Request<RecvStream>,
    mut respond: SendResponse<Bytes>,
    shared: Arc<Shared>,
) {
    let at = std::time::Instant::now();
    let post = request.method() == http::Method::POST;
    let mut incoming = request.into_body();
    let mut body = Vec::new();
    while let Some(chunk) = incoming.data().await {
        let Ok(chunk) = chunk else {
            return;
        };
        if incoming
            .flow_control()
            .release_capacity(chunk.len())
            .is_err()
        {
            return;
        }
        body.extend_from_slice(&chunk);
    }
    let request = Request {
        post,
        body,
        at,
        via: Transport::H2,
    };
    let Some(reply) = shared.arrive(request) else {
        return;
    };
    let head = http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, reply.content_type())
        .body(());
    let Ok(head) = head else {
        return;
    };
    let Ok(mut outgoing) = respond.send_response(head, false) else {
        return;
    };
    let pieces = reply.pieces();
    let last = pieces.len().saturating_sub(1);
    for (n, piece) in pieces.into_iter().enumerate() {
        if let Some(gap) = reply.pace.filter(|_| n > 0) {
            tokio::time::sleep(gap).await;
        }
        let data = Bytes::copy_from_slice(piece.as_bytes());
        if outgoing.send_data(data, n == last).is_err() {
            return;
        }
    }
    reply.written();
}

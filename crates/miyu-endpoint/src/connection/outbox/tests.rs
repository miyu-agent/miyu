//! 写的一头（施工 V-22）：几行一起写，字节和先后同一行行写的一样；排着的才一起写，只冲一次；攒多了分几次；写不出去就停。

use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::io::AsyncWrite;
use tokio::sync::mpsc;

use super::{BATCH, write_all};

/// 记下写了什么、冲了几次的写端；`broken` 的一写就出错。
#[derive(Clone, Default)]
struct Wire {
    seen: Arc<Mutex<Seen>>,
    broken: bool,
}

#[derive(Default)]
struct Seen {
    bytes: Vec<u8>,
    flushes: usize,
}

impl Wire {
    fn bytes(&self) -> Vec<u8> {
        self.seen.lock().unwrap().bytes.clone()
    }

    fn flushes(&self) -> usize {
        self.seen.lock().unwrap().flushes
    }
}

impl AsyncWrite for Wire {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.broken {
            return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
        }
        self.seen.lock().unwrap().bytes.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.seen.lock().unwrap().flushes += 1;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// 一行行写会写出的字节。
fn joined(lines: &[String]) -> Vec<u8> {
    lines
        .iter()
        .flat_map(|line| format!("{line}\n").into_bytes())
        .collect()
}

/// 回应和推送从两个发送端交错着进同一个队列：写出去的照入队的先后，字节同一行行写的一样；先排好的一次冲完。
#[tokio::test]
async fn queued_lines_go_out_together_in_queue_order() {
    let (out, lines) = mpsc::channel(256);
    let pushes = out.clone();
    let mut sent = Vec::new();
    for n in 0..100 {
        let (sender, line) = match n % 3 {
            0 => (&out, format!(r#"{{"id":"c{n}","result":{{}}}}"#)),
            _ => (
                &pushes,
                format!(r#"{{"method":"session.delta","params":{{"n":{n}}}}}"#),
            ),
        };
        sender.send(line.clone()).await.unwrap();
        sent.push(line);
    }
    drop((out, pushes));
    let wire = Wire::default();
    write_all(wire.clone(), lines).await;
    assert_eq!(wire.bytes(), joined(&sent));
    assert_eq!(wire.flushes(), 1, "排着的一百行一次冲");
}

/// 没有排着的：来一行写一行，不等后面的。
#[tokio::test]
async fn a_lone_line_goes_out_without_waiting() {
    let (out, lines) = mpsc::channel(256);
    let wire = Wire::default();
    let writer = tokio::spawn(write_all(wire.clone(), lines));
    out.send("first".to_string()).await.unwrap();
    for _ in 0..100 {
        if wire.flushes() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(wire.bytes(), b"first\n", "后面还没来就先写出去");
    out.send("second".to_string()).await.unwrap();
    drop(out);
    writer.await.unwrap();
    assert_eq!(wire.bytes(), b"first\nsecond\n");
    assert_eq!(wire.flushes(), 2);
}

/// 攒到上限就先写：大的几行分几次，字节和先后照旧。
#[tokio::test]
async fn a_big_backlog_goes_out_in_pieces_unchanged() {
    let (out, lines) = mpsc::channel(256);
    let line = "x".repeat(BATCH / 4);
    let sent: Vec<String> = (0..12).map(|n| format!("{n}{line}")).collect();
    for line in &sent {
        out.send(line.clone()).await.unwrap();
    }
    drop(out);
    let wire = Wire::default();
    write_all(wire.clone(), lines).await;
    assert_eq!(wire.bytes(), joined(&sent));
    assert!(wire.flushes() >= 3, "攒不下的分几次写：{}", wire.flushes());
}

/// 写不出去就停，不再收队列。
#[tokio::test]
async fn a_broken_wire_stops_the_writer() {
    let (out, lines) = mpsc::channel(256);
    out.send("lost".to_string()).await.unwrap();
    let wire = Wire {
        broken: true,
        ..Wire::default()
    };
    write_all(wire.clone(), lines).await;
    assert!(
        out.send("after".to_string()).await.is_err(),
        "写端停了，队列跟着关"
    );
    assert!(wire.bytes().is_empty());
}

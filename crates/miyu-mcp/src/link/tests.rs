//! 写的队列、进度通知的队列有上限（施工 AE-8，发布前第二轮审查 R2-9）：服务不读标准输入、跟进度的那一头不收，核心的内存不
//! 跟着涨。满了的通知丢掉；请求照等回应的时限等空位。

use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use super::{Link, QUEUE};
use crate::wire;

/// 起一条连接：交回连接、服务那一头的标准输入（读它就是收核心写的）、服务那一头的标准输出（写它就是服务说话）。
fn linked() -> (Link, DuplexStream, DuplexStream) {
    let (writer, server_in) = tokio::io::duplex(64);
    let (server_out, reader) = tokio::io::duplex(64);
    (Link::start(reader, writer), server_in, server_out)
}

/// 把服务那一头的标准输入读到停了 300 毫秒没有新的，数有几行。
async fn lines_until_quiet(server_in: &mut DuplexStream) -> usize {
    let mut seen = Vec::new();
    let mut buffer = [0u8; 4096];
    while let Ok(Ok(read)) =
        tokio::time::timeout(Duration::from_millis(300), server_in.read(&mut buffer)).await
    {
        if read == 0 {
            break;
        }
        seen.extend_from_slice(&buffer[..read]);
    }
    seen.iter().filter(|byte| **byte == b'\n').count()
}

#[tokio::test]
async fn notices_to_a_server_that_does_not_read_do_not_pile_up() {
    let (link, mut server_in, _server_out) = linked();
    for n in 0..10_000 {
        link.tell("notifications/message", json!({ "n": n }));
    }
    let delivered = lines_until_quiet(&mut server_in).await;
    // 队列里的、写的那一头正拿着的、管道里塞着的几行：比上限多不了几条。
    assert!(
        delivered <= QUEUE + 8,
        "服务不读的时候发了一万条通知，后来照样送到了 {delivered} 条：都攒在内存里"
    );
}

#[tokio::test]
async fn progress_nobody_takes_does_not_pile_up() {
    let (link, _server_in, mut server_out) = linked();
    let token = json!("audit");
    let (mut updates, _following) = link.follow(&token);
    for n in 0..10_000 {
        let line = wire::notice(
            "notifications/progress",
            json!({ "progressToken": token, "progress": n }),
        );
        server_out.write_all(&line).await.expect("写得进");
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut kept = 0;
    while updates.try_recv().is_ok() {
        kept += 1;
    }
    assert!(
        kept <= QUEUE,
        "跟进度的那一头一直不收，服务发了一万条进度，攒下了 {kept} 条"
    );
}

#[tokio::test]
async fn a_request_waits_for_room_only_as_long_as_it_waits_for_the_answer() {
    let (link, _server_in, _server_out) = linked();
    for n in 0..(QUEUE * 2) {
        link.tell("notifications/message", json!({ "n": n }));
    }
    let started = std::time::Instant::now();
    let asked = link
        .ask(
            "tools/list",
            json!({}),
            Some(Duration::from_millis(200)),
            false,
        )
        .await;
    assert!(
        matches!(asked, Err(crate::error::Failed::TimedOut)),
        "{asked:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "排空位不能超过等回应的时限"
    );
}

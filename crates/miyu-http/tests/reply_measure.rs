//! 量尺（施工 PF-2）：CPU 被抢时，对方早把回复的字节全写进了连接，核心要隔多久才「知道」发完、收完。只量不断言。
//!
//! 照压测（bm「组合加码」，2026-11-13）的假模型写：收齐请求体就回响应头，再一帧 SSE 一个 HTTP 分块、一帧一次 `write`（关了
//! Nagle），长会话一步的回复就是六七帧，零点几毫秒写完。发的一方照核心：`send` 在派出去的任务里跑，交出来的经无界通道送给
//! 另一个任务（替 actor）。量四段：
//!
//! - 收齐 → 交出「发出去了」：对方收齐请求体，到 `send` 交出 `Progress::Sent`（核心的 `request_sent` 再晚一跳）；
//! - 收齐 → 替身收到「发出去了」；
//! - 回完 → `send` 收场：对方写完最后一个字节，到 `send` 交回（核心的 `http ended`）；
//! - 回完 → 替身收到最后一样。
//!
//! 跑法：`cargo test --release -p miyu-http --test all --no-run`，把测试程序和压力（bm 的 `tools/pressure.c` 原样编）挤在同两个
//! 核上：`taskset -c <两个核> pressure cpu 3 600 &`、`taskset -c <两个核> pressure thr 34 600 &`（约第 1 档摊到两个核上；第 2 档
//! 11、167），再 `taskset -c <两个核> target/release/deps/all-<哈希> reply_measure --ignored --nocapture`。轮数照
//! `MIYU_PF2_ROUNDS`（默认 30），请求体多大照 `MIYU_PF2_BYTES`（默认 400000，压测长会话中段一步的大小），运行时几个工作线程
//! 照 `MIYU_PF2_WORKERS`（默认 2：核心照核数开，这里两个核），运行时里另起几个忙着的任务照 `MIYU_PF2_BUSY`（默认 0，见
//! [`busy_task`]）。

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::support::{driver, sample};
use miyu_http::{Attempt, Endpoint, Outcome, Progress, Proxy, client, send};
use tokio::sync::mpsc;

/// 对方那一头记下的两个时刻：收齐请求体、写完最后一个字节。
type Marks = (Instant, Instant);

/// 回复：样本里的六帧，一帧一个分块。
fn frames() -> Vec<Vec<u8>> {
    let stream = sample("openai-text");
    let text = String::from_utf8(stream).expect("样本是 UTF-8");
    // 样本是 CRLF 的：照空行切，切出来的每一帧照原样带上自己的空行。
    text.split_inclusive("\r\n\r\n")
        .filter(|frame| !frame.trim().is_empty())
        .map(|frame| frame.as_bytes().to_vec())
        .collect()
}

/// 收的一头：照 `Content-Length` 读完请求体，回响应头，再一帧一次 `write`；连接留着接着用。
fn serve(listener: TcpListener, marks: std_mpsc::Sender<Marks>) {
    let frames = frames();
    for connection in listener.incoming() {
        let Ok(connection) = connection else { return };
        let (marks, frames) = (marks.clone(), frames.clone());
        thread::spawn(move || answer(connection, &marks, &frames));
    }
}

fn answer(connection: TcpStream, marks: &std_mpsc::Sender<Marks>, frames: &[Vec<u8>]) {
    connection.set_nodelay(true).expect("关得掉 Nagle");
    let mut writer = connection.try_clone().expect("拷得出写的一头");
    let mut reader = BufReader::with_capacity(256 * 1024, connection);
    loop {
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().expect("长度是数");
            }
        }
        let mut body = vec![0u8; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let received = Instant::now();
        let head = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n";
        if writer.write_all(head).is_err() {
            return;
        }
        for frame in frames {
            let mut chunk = format!("{:x}\r\n", frame.len()).into_bytes();
            chunk.extend_from_slice(frame);
            chunk.extend_from_slice(b"\r\n");
            if writer.write_all(&chunk).is_err() {
                return;
            }
        }
        if writer.write_all(b"0\r\n\r\n").is_err() {
            return;
        }
        if marks.send((received, Instant::now())).is_err() {
            return;
        }
    }
}

/// 交给替身的：哪一样、`send` 交出来的那一刻。
enum Mark {
    Sent(Instant),
    Delta,
    End,
}

/// 一轮的四段。
struct Round {
    sent: Duration,
    sent_seen: Duration,
    ended: Duration,
    ended_seen: Duration,
    deltas: usize,
}

/// 一轮：在派出去的任务里发，替身在另一个任务里收。
async fn round(endpoint: &Endpoint, body: &[u8], marks: &std_mpsc::Receiver<Marks>) -> Round {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let actor = tokio::spawn(async move {
        let (mut sent, mut deltas) = (None, 0usize);
        while let Some(mark) = rx.recv().await {
            let at = Instant::now();
            match mark {
                Mark::Sent(on) => sent = Some((on, at)),
                Mark::Delta => deltas += 1,
                Mark::End => return (sent.expect("先报发出去了"), at, deltas),
            }
        }
        panic!("没收到最后一样");
    });
    let (endpoint, body) = (endpoint.clone(), body.to_vec());
    let ended = tokio::spawn(async move {
        let client = client(Proxy::Off).expect("造得出客户端");
        let driver = driver();
        let outcome = send(
            Attempt {
                client: &client,
                endpoint: &endpoint,
                driver: &driver,
                body: body.into(),
                path: "/chat/completions",
                idle: Duration::from_secs(30),
            },
            std::future::pending(),
            |progress| {
                let mark = match progress {
                    Progress::Sent { .. } => Mark::Sent(Instant::now()),
                    Progress::Delta(_) => Mark::Delta,
                };
                // 替身不收了（量完了）就不交。
                drop(tx.send(mark));
            },
        )
        .await;
        let ended = Instant::now();
        assert!(
            matches!(outcome, Outcome::Ended { error: None, .. }),
            "{outcome:?}"
        );
        drop(tx.send(Mark::End));
        ended
    })
    .await
    .expect("发的任务没崩");
    let ((sent, sent_seen), ended_seen, deltas) = actor.await.expect("替身没崩");
    let (received, done) = marks.recv().expect("对方记下了时刻");
    Round {
        sent: sent.saturating_duration_since(received),
        sent_seen: sent_seen.saturating_duration_since(received),
        ended: ended.saturating_duration_since(done),
        ended_seen: ended_seen.saturating_duration_since(done),
        deltas,
    }
}

fn env_number(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// 运行时里另有的活（替别的会话）：算 200 微秒、睡 1 毫秒，一直这样。核心里几十个会话各有各的任务，运行时的工作线程很少闲着。
async fn busy_task() {
    loop {
        let until = Instant::now() + Duration::from_micros(200);
        while Instant::now() < until {
            std::hint::spin_loop();
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// 中位、90 分位，毫秒。
fn spread(mut values: Vec<Duration>) -> String {
    values.sort();
    let at = |q: usize| values[values.len() * q / 10].as_secs_f64() * 1e3;
    format!("中位 {:>7.2} ms，90 分位 {:>7.2} ms", at(5), at(9))
}

#[test]
#[ignore = "量尺，只量不断言"]
fn reply_measure() {
    let rounds = env_number("MIYU_PF2_ROUNDS", 30);
    let bytes = env_number("MIYU_PF2_BYTES", 400_000);
    let workers = env_number("MIYU_PF2_WORKERS", 2);
    let busy = env_number("MIYU_PF2_BUSY", 0);
    let listener = TcpListener::bind("127.0.0.1:0").expect("占得到端口");
    let endpoint = Endpoint::keyless(format!("http://{}", listener.local_addr().expect("有地址")));
    let (sender, marks) = std_mpsc::channel();
    thread::spawn(move || serve(listener, sender));
    let mut body = br#"{"messages":[{"role":"user","content":""#.to_vec();
    body.resize(bytes.saturating_sub(4), b'x');
    body.extend_from_slice(br#""}]}"#);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .expect("造得出运行时");
    for _ in 0..busy {
        runtime.spawn(busy_task());
    }
    let results = runtime.block_on(async {
        for _ in 0..2 {
            round(&endpoint, &body, &marks).await;
        }
        let mut results = Vec::new();
        for _ in 0..rounds {
            results.push(round(&endpoint, &body, &marks).await);
        }
        results
    });
    println!(
        "请求体 {} 字节，回 {} 帧（交出 {} 段增量），{workers} 个工作线程，另有 {busy} 个忙着的任务，{rounds} 轮",
        body.len(),
        frames().len(),
        results[0].deltas
    );
    let pick = |f: fn(&Round) -> Duration| results.iter().map(f).collect::<Vec<_>>();
    println!("收齐 → 交出发出去了：{}", spread(pick(|r| r.sent)));
    println!("收齐 → 替身收到：    {}", spread(pick(|r| r.sent_seen)));
    println!("回完 → send 收场：   {}", spread(pick(|r| r.ended)));
    println!("回完 → 替身收到：    {}", spread(pick(|r| r.ended_seen)));
}

//! 量尺（施工 PF-1）：CPU 被抢时，发一个 2 MB 上下、夹着 24 张小图的请求体要多久，交给 HTTP 客户端几块、其间等了阻塞线程几回。
//! 只量不断言，跑法（先编好，再把测试程序挤在两个核上跑；进程里另起压力，见 [`Pressure`]）：
//!
//! `cargo test --release -p miyu-http --lib --no-run`，再
//! `taskset -c <两个核> target/release/deps/miyu_http-<哈希> pf1_body_under_cpu_pressure --ignored --nocapture`
//!
//! 压力照 `MIYU_PF1_CPU`（一直算的线程，默认 11）、`MIYU_PF1_TICK`（睡醒的线程，默认 167），是压测「组合加码」第 2 档
//! （64、1000 条摊在 12 个核上）摊到两个核上的量；量几轮照 `MIYU_PF1_ROUNDS`（默认 30）。请求体照压测的长会话（bm 的「组合加码」，
//! 2026-11-13）：图是工具读回来的截图，一张 58 KB 上下，图和图之间是几 KB 到十几 KB 的字。收的一头照压测的假模型
//! 一次读 256 KiB，读完整份记下时刻；量的是从交给客户端到对方收齐。

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// 图几张、一张多大、两张之间几个字。
const PICTURES: usize = 24;
const PICTURE: usize = 58_000;
const BETWEEN: usize = 15_000;

/// 照压测长会话的样子拼的请求体（`tests.rs` 里查块数的那一条也用它）。
pub(super) fn bench_body() -> (Fragments, Memory) {
    let mut memory = Memory::default();
    let mut parts = vec![Fragment::Text(b"{\"messages\":[".to_vec())];
    for index in 0..PICTURES {
        let bytes = picture(PICTURE, u32::try_from(index).unwrap());
        let hash = ContentHash::of(&bytes);
        memory.blobs.insert(hash.clone(), bytes);
        let mut text = b"{\"role\":\"tool\",\"content\":\"".to_vec();
        text.extend(std::iter::repeat_n(b'x', BETWEEN));
        text.extend_from_slice(b"\"},{\"role\":\"user\",\"content\":[{\"type\":\"image_url\",\"image_url\":{\"url\":\"data:image/png;base64,");
        parts.push(Fragment::Text(text));
        parts.push(Fragment::Base64 {
            blob: hash,
            size: u64::try_from(PICTURE).unwrap(),
        });
        parts.push(Fragment::Text(b"\"}}]},".to_vec()));
    }
    parts.push(Fragment::Text(
        b"{\"role\":\"user\",\"content\":\"go\"}]}".to_vec(),
    ));
    (parts.into_iter().collect(), memory)
}

/// 数一条流交了几块、在 `Pending` 上停了几回（每停一回，就要等别的线程把它叫醒、再排上队）。
struct Counting {
    inner: Stream,
    frames: Arc<AtomicUsize>,
    pendings: Arc<AtomicUsize>,
}

impl http_body::Body for Counting {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        let polled = Pin::new(&mut self.inner).poll_frame(cx);
        match &polled {
            Poll::Pending => self.pendings.fetch_add(1, Ordering::Relaxed),
            Poll::Ready(Some(Ok(_))) => self.frames.fetch_add(1, Ordering::Relaxed),
            Poll::Ready(_) => 0,
        };
        polled
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

/// 收的一头：照 `Content-Length` 一次读 256 KiB 读完，记下收齐的时刻，回一个空的 JSON；连接留着接着用。
fn serve(listener: TcpListener, done: std_mpsc::Sender<Instant>) {
    for connection in listener.incoming() {
        let Ok(connection) = connection else { return };
        let done = done.clone();
        thread::spawn(move || answer(connection, &done));
    }
}

fn answer(connection: TcpStream, done: &std_mpsc::Sender<Instant>) {
    let mut writer = connection.try_clone().unwrap();
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
                length = value.trim().parse().unwrap();
            }
        }
        let mut body = vec![0u8; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        if done.send(Instant::now()).is_err() {
            return;
        }
        let reply =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        if writer.write_all(reply).is_err() {
            return;
        }
    }
}

/// 一轮：交给客户端到对方收齐多久，交了几块、停了几回。
struct Round {
    took: Duration,
    frames: usize,
    pendings: usize,
}

async fn round(
    client: &reqwest::Client,
    url: &str,
    body: &(Fragments, Arc<Memory>),
    done: &std_mpsc::Receiver<Instant>,
) -> Round {
    let replay = Arc::new(Replay::new(Body::new(body.0.clone(), body.1.clone())));
    let frames = Arc::new(AtomicUsize::new(0));
    let pendings = Arc::new(AtomicUsize::new(0));
    let counting = Counting {
        inner: replay.stream(),
        frames: Arc::clone(&frames),
        pendings: Arc::clone(&pendings),
    };
    let mut request = client
        .post(url)
        .header("Content-Type", "application/json")
        .build()
        .unwrap();
    *request.body_mut() = Some(reqwest::Body::wrap(counting));
    let started = Instant::now();
    let response = client.execute(request).await.unwrap();
    assert!(response.status().is_success());
    response.bytes().await.unwrap();
    let received = done.recv().unwrap();
    Round {
        took: received.saturating_duration_since(started),
        frames: frames.load(Ordering::Relaxed),
        pendings: pendings.load(Ordering::Relaxed),
    }
}

fn env_number(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// 进程里另起的压力，照压测的 `pressure`（bm 的 `tools/pressure.c`）：`cpu` 条一直算的线程，`tick` 条睡 1 ms、醒来算一小会儿的
/// 线程（排队的人多）。压测的压力和被测的程序在同一个没开 CPU 控制器的 slice 里，线程平着抢，这里同一个进程也一样。
struct Pressure {
    stop: Arc<AtomicBool>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl Pressure {
    fn start(cpu: usize, tick: usize) -> Pressure {
        let stop = Arc::new(AtomicBool::new(false));
        let spawn = |ticking: bool| {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                let mut x = 1u64;
                while !stop.load(Ordering::Relaxed) {
                    if ticking {
                        thread::sleep(Duration::from_millis(1));
                    }
                    for _ in 0..if ticking { 20_000 } else { 100_000 } {
                        x = std::hint::black_box(
                            x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1),
                        );
                    }
                }
            })
        };
        let threads = (0..cpu)
            .map(|_| spawn(false))
            .chain((0..tick).map(|_| spawn(true)))
            .collect();
        Pressure { stop, threads }
    }

    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        for thread in self.threads {
            thread.join().unwrap();
        }
    }
}

/// 量一组：先热两轮（连接、阻塞线程），再量 `rounds` 轮，印中位、90 分位。
fn measure(label: &str, rounds: usize, body: &(Fragments, Arc<Memory>)) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/chat/completions", listener.local_addr().unwrap());
    let (sender, done) = std_mpsc::channel();
    thread::spawn(move || serve(listener, sender));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let client = crate::client(crate::Proxy::Off).unwrap();
    let mut results = runtime.block_on(async {
        for _ in 0..2 {
            round(&client, &url, body, &done).await;
        }
        let mut results = Vec::new();
        for _ in 0..rounds {
            results.push(round(&client, &url, body, &done).await);
        }
        results
    });
    results.sort_by_key(|round| round.took);
    let median = &results[results.len() / 2];
    let p90 = &results[results.len() * 9 / 10];
    let frames: Vec<usize> = results.iter().map(|round| round.frames).collect();
    let mut pendings: Vec<usize> = results.iter().map(|round| round.pendings).collect();
    pendings.sort_unstable();
    println!(
        "{label}：中位 {:>7.1} ms，90 分位 {:>7.1} ms；交了 {}～{} 块，停了中位 {} 回",
        median.took.as_secs_f64() * 1e3,
        p90.took.as_secs_f64() * 1e3,
        frames.iter().min().unwrap(),
        frames.iter().max().unwrap(),
        pendings[pendings.len() / 2],
    );
}

#[test]
#[ignore = "量尺，只量不断言"]
fn pf1_body_under_cpu_pressure() {
    let (fragments, memory) = bench_body();
    println!(
        "请求体 {} 字节，{} 段，{PICTURES} 张图",
        fragments.len(),
        fragments.parts().len()
    );
    // 对照：同样的字节整份一段字（没有 blob，一块交完），剩下的就是收的一头和 TCP 来回，不归请求体。
    let flat = Fragments::from(whole(&fragments, &memory));
    let bodies = [
        ("夹图的", (fragments, Arc::new(memory))),
        ("整份的", (flat, Arc::new(Memory::default()))),
    ];
    let rounds = env_number("MIYU_PF1_ROUNDS", 30);
    let pressed = (
        env_number("MIYU_PF1_CPU", 11),
        env_number("MIYU_PF1_TICK", 167),
    );
    for (cpu, tick) in [(0, 0), pressed] {
        let pressure = Pressure::start(cpu, tick);
        for (name, body) in &bodies {
            measure(
                &format!("烧 {cpu:>2} 条、睡醒 {tick:>3} 条，{name}"),
                rounds,
                body,
            );
        }
        pressure.stop();
    }
}

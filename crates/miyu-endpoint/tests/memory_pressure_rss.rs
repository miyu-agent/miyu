//! 量尺（施工 MEM-3）：内存紧时卸掉一批闲着的会话，核心的常驻降多少、卸那一批花多久。量的是整个进程的内存，和别的测试并在
//! 一起会把它们分配的也算进来，所以单独一个测试程序（`AGENTS.md`「测试」）。标 `#[ignore]`：只打印数，不断言多少。
//!
//! 卸的期间另一个连接每 10 毫秒列一次会话（`session.list`，要拿会话表的锁），记它最久等了多久（施工 MEM-5）。挤核的量法：
//! 整个测试套 `taskset -c 0`，同一个核上另起烧 CPU 的进程。运行队列挤满的量法：`MIYU_RSS_CROWD` 个任务在卸的期间轮流占着
//! 工作线程（每个占 1 毫秒再排回队尾，像一批回合同时在跑）。
//!
//! 照核心的样子先调好分配器（`miyu_heap::tune`，`miyu-core` 起来时最先调），运行时是多线程的（4 个工作线程）。
//!
//! 跑法：`cargo test -p miyu-endpoint --test memory_pressure_rss -- --ignored --nocapture`；会话数照 `MIYU_RSS_SESSIONS`
//! （默认 1000）。

mod support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use miyu_heap::room::{Room, Rooms};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::*;

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// 这一刻的常驻、匿名内存（MiB）和 glibc 堆里在用的、空着的（MiB）。
fn measure(what: &str) -> u64 {
    let usage = miyu_heap::usage::usage();
    let heap = miyu_heap::stats();
    let rss = usage.map_or(0, |usage| usage.rss);
    // 线程数（Linux）：每个退下的会话各排一次 trim 时，阻塞线程池会一下子起很多线程。
    let threads = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("Threads:").map(|n| n.trim().to_string()))
        })
        .unwrap_or_default();
    println!(
        "{what}: threads={threads} rss={} MiB anon={} MiB heap_used={} MiB heap_free={} MiB",
        rss / MIB,
        usage.and_then(|usage| usage.anon).unwrap_or(0) / MIB,
        heap.map_or(0, |heap| heap.used) / MIB,
        heap.map_or(0, |heap| heap.free) / MIB,
    );
    rss
}

#[test]
#[ignore = "量尺：打印卸一批会话前后的内存，不断言"]
fn retiring_a_batch_gives_memory_back() {
    assert!(
        miyu_heap::tune() || !cfg!(target_env = "gnu"),
        "调得好分配器"
    );
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("建得了运行时")
        .block_on(retire_a_batch());
}

async fn retire_a_batch() {
    let count = env_count("MIYU_RSS_SESSIONS", 1000);
    let home = Home::new();
    let tight = Arc::new(Mutex::new(false));
    let reading = Arc::clone(&tight);
    let script =
        Script::new(vec![Play::Says("好。"); count]).titles(vec![Play::Says("测试"); count]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_sandbox(miyu_sandbox::Availability::Usable("miyu-sandbox".into()))
            .with_memory_probe(
                Duration::from_millis(50),
                move || {
                    let tight = *reading.lock().unwrap_or_else(PoisonError::into_inner);
                    let available = if tight { 3 * GIB } else { 50 * GIB };
                    Rooms {
                        system: None,
                        cgroup: Some(Room {
                            available,
                            total: 100 * GIB,
                        }),
                    }
                },
                || None,
            ),
    );
    tokio::task::spawn_blocking(miyu_heap::trim)
        .await
        .expect("还得了");
    let empty = measure("没开会话");
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let mut ids = Vec::with_capacity(count);
    for n in 0..count {
        let id = client.create(&format!("c{n}"), "~").await;
        client.say(&format!("s{n}"), &id, "你好").await;
        ids.push(id);
    }
    for id in &ids {
        home.until_turns(id, 1).await;
    }
    drop(client);
    tokio::time::sleep(Duration::from_secs(1)).await;
    tokio::task::spawn_blocking(miyu_heap::trim)
        .await
        .expect("还得了");
    let loaded = measure(&format!("载入着 {} 个", core.loaded().await));
    let mut lister = Client::connect(core.clone());
    lister.hello().await;
    // 不卸的时候列一次会话多久：卸的期间等得比它久的才是等锁、等堆。
    let mut quiet = Duration::ZERO;
    for n in 0..5 {
        let asked = Instant::now();
        lister
            .call(
                &format!("q{n}"),
                "session.list",
                serde_json::json!({"limit": 1}),
            )
            .await;
        quiet = quiet.max(asked.elapsed());
    }
    println!("不卸的时候列一次会话最久 {quiet:?}");
    let done = Arc::new(AtomicBool::new(false));
    let listing = tokio::spawn({
        let done = Arc::clone(&done);
        async move {
            let (mut longest, mut listed) = (Duration::ZERO, 0_usize);
            while !done.load(Ordering::Acquire) {
                let asked = Instant::now();
                let reply = lister
                    .call(
                        &format!("l{listed}"),
                        "session.list",
                        serde_json::json!({"limit": 1}),
                    )
                    .await;
                assert!(reply.get("result").is_some(), "{reply}");
                longest = longest.max(asked.elapsed());
                listed += 1;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            (longest, listed)
        }
    });
    let crowd: Vec<_> = (0..env_count("MIYU_RSS_CROWD", 0))
        .map(|_| {
            tokio::spawn(async {
                loop {
                    std::thread::sleep(Duration::from_millis(1));
                    queue_again().await;
                }
            })
        })
        .collect();
    let started = Instant::now();
    *tight.lock().unwrap_or_else(PoisonError::into_inner) = true;
    while core.loaded().await > 0 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let took = started.elapsed();
    for hog in crowd {
        hog.abort();
    }
    done.store(true, Ordering::Release);
    let (longest, listed) = listing.await.expect("列得完");
    println!("卸完 {count} 个用了 {took:?}；期间列了 {listed} 次会话，最久等了 {longest:?}");
    for after in [1, 2, 5, 12] {
        let since = started.elapsed();
        tokio::time::sleep(Duration::from_secs(after).saturating_sub(since)).await;
        measure(&format!("卸完 {after} 秒"));
    }
    let retired = measure("卸完 12 秒");
    tokio::task::spawn_blocking(miyu_heap::trim)
        .await
        .expect("还得了");
    let trimmed = measure("再 trim 一次");
    println!(
        "一个会话约 {} KiB；卸掉以后还回去 {} MiB，比没开会话多 {} MiB（再 trim 一次以后多 {} MiB）",
        loaded.saturating_sub(empty) / 1024 / u64::try_from(count).unwrap_or(1).max(1),
        loaded.saturating_sub(retired) / MIB,
        retired.saturating_sub(empty) / MIB,
        trimmed.saturating_sub(empty) / MIB,
    );
}

/// 环境变量 `name` 写的数，没写、写错的是 `default`。
fn env_count(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|count| count.parse().ok())
        .unwrap_or(default)
}

/// 让出线程、排回运行队列的末尾（`tokio::task::yield_now` 让出的要等队列空了才再跑，占不住别人的队）。
async fn queue_again() {
    let mut queued = false;
    std::future::poll_fn(|cx| {
        if queued {
            std::task::Poll::Ready(())
        } else {
            queued = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await;
}

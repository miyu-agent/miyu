use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::spawn::{hear, launch};
use super::*;
use crate::heard::first_line;
use crate::lock::Lock;
use crate::test_support::Scratch;

/// 一段小程序：Unix 上照 `sh -c`，Windows 上照 `cmd /C`。
fn script(unix: &str, windows: &str) -> Command {
    if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", windows]);
        command
    } else {
        let mut command = Command::new("sh");
        command.args(["-c", unix]);
        command
    }
}

/// 一直不说话、过 `seconds` 秒才退的小程序。
fn silent_for(seconds: u32) -> Command {
    script(
        &format!("sleep {seconds}"),
        &format!("ping -n {} 127.0.0.1 >NUL", seconds + 1),
    )
}

/// 叫了几次「慢了」。
fn counter() -> (Arc<AtomicUsize>, impl Fn() + Send + Sync) {
    let told = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&told);
    (told, move || {
        counted.fetch_add(1, Ordering::Relaxed);
    })
}

/// 测试里的耐心：`slow`、`limit` 毫秒。
fn patience(slow: u64, limit: u64) -> Patience {
    Patience {
        slow: Duration::from_millis(slow),
        limit: Duration::from_millis(limit),
    }
}

#[tokio::test]
async fn each_line_the_core_writes_is_heard() {
    for (line, heard) in [
        ("ready\n", Ready::Ready),
        ("running\n", Ready::Running),
        ("error 找不到资源目录\n", Ready::failed("找不到资源目录")),
    ] {
        let (reader, mut writer) = io::pipe().expect("开得了管道");
        writer.write_all(line.as_bytes()).expect("写得进");
        let (_alive, gone) = tokio::sync::oneshot::channel();
        let clock = Clock::new(Patience::DEFAULT, QUIET);
        let got = hear(first_line(reader), gone, &clock).await;
        assert_eq!(got.expect("听得到"), heard);
    }
}

#[tokio::test]
async fn a_core_that_leaves_without_a_word_is_silent() {
    let (reader, writer) = io::pipe().expect("开得了管道");
    drop(writer);
    let (_alive, gone) = tokio::sync::oneshot::channel();
    let clock = Clock::new(Patience::DEFAULT, QUIET);
    let error = hear(first_line(reader), gone, &clock)
        .await
        .expect_err("没说话");
    assert!(matches!(error, StartError::Silent), "{error:?}");
}

#[tokio::test]
async fn a_core_that_says_nothing_for_too_long_times_out() {
    let (reader, writer) = io::pipe().expect("开得了管道");
    let (_alive, gone) = tokio::sync::oneshot::channel();
    let clock = Clock::new(patience(50, 100), QUIET);
    let error = hear(first_line(reader), gone, &clock)
        .await
        .expect_err("等太久");
    assert!(matches!(error, StartError::Timeout), "{error:?}");
    drop(writer);
}

/// 等过了上限、不再听以后，放下运行时不用等那个一直不说话的程序（施工 CS-6）：原来读的那一段在阻塞线程池里，放下运行时要等它
/// 读到东西，头 60 秒到了也要等核心写出那一行才退得了。
#[test]
fn giving_up_does_not_hold_the_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("建得了运行时");
    let scratch = Scratch::new();
    let started = Instant::now();
    let clock = Clock::new(patience(10_000, 300), QUIET);
    let error = runtime
        .block_on(launch(silent_for(10), scratch.path(), &clock))
        .expect_err("等太久");
    assert!(matches!(error, StartError::Timeout), "{error:?}");
    drop(runtime);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// 起得慢的：过了 `slow` 叫一声，只叫一次，接着等到它说好了。
#[tokio::test]
async fn a_slow_start_is_told_once_and_still_waited_for() {
    let (told, slow) = counter();
    let scratch = Scratch::new();
    let clock = Clock::new(patience(200, 30_000), &slow);
    let command = script(
        "sleep 1; echo ready",
        "ping -n 2 127.0.0.1 >NUL & echo ready",
    );
    let heard = launch(command, scratch.path(), &clock).await;
    assert_eq!(heard.expect("等到了"), Ready::Ready);
    assert_eq!(told.load(Ordering::Relaxed), 1);
}

/// 说了起不来的当场结束，不等到上限。
#[tokio::test]
async fn a_refusal_ends_the_wait_at_once() {
    let scratch = Scratch::new();
    let clock = Clock::new(patience(10_000, 60_000), QUIET);
    let started = Instant::now();
    let heard = launch(
        script("echo 'error boom'", "echo error boom"),
        scratch.path(),
        &clock,
    )
    .await;
    assert_eq!(heard.expect("听得到"), Ready::failed("boom"));
    assert!(started.elapsed() < Duration::from_secs(10));
}

/// 拉起的进程退了、它起的子进程还拿着管道：再听一小会儿就当没说话就退了，不等子进程、不等到上限（施工 CS-6：看进程在不在，不只看
/// 管道）。
#[cfg(unix)]
#[tokio::test]
async fn a_process_that_left_is_not_waited_for_through_its_children() {
    let scratch = Scratch::new();
    let clock = Clock::new(patience(10_000, 60_000), QUIET);
    let started = Instant::now();
    let heard = launch(script("sleep 10 & exit 0", ""), scratch.path(), &clock).await;
    assert!(matches!(heard, Err(StartError::Silent)), "{heard:?}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// 连不上的当没在跑：交给 [`start_with`] 的连法。
async fn unreachable(_root: &DataRoot) -> Result<(), ConnectError> {
    Err(ConnectError::NotRunning)
}

/// 别的头一直拿着拉起锁：慢了叫一声，到了上限是 `Busy`，自己不拉。
#[tokio::test]
async fn another_head_starting_for_too_long_is_busy() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let other = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.run().join(SPAWN_LOCK))
        .expect("打得开");
    other.try_lock().expect("拿得到");
    let (told, slow) = counter();
    let clock = Clock::new(patience(100, 400), &slow);
    let never = || -> Command { panic!("拉起锁在别人手里，不拉") };
    let result = start_with(&root, Starting::Asked, never, unreachable, &clock).await;
    assert!(matches!(result, Err(StartError::Busy)), "{result:?}");
    assert_eq!(told.load(Ordering::Relaxed), 1);
}

/// 核心的锁在别人手里、又连不上（正在起来或者正在收尾）：等它，慢了叫一声；锁放了再拉。
#[tokio::test]
async fn a_core_still_holding_the_lock_is_waited_out_before_starting_another() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let lock = Lock::acquire(&root).expect("拿得到");
    let (told, slow) = counter();
    let clock = Clock::new(patience(100, 30_000), &slow);
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        drop(lock);
    });
    let started = Instant::now();
    let launched = || script("echo 'error launched'", "echo error launched");
    let result = start_with(&root, Starting::Asked, launched, unreachable, &clock).await;
    assert!(
        matches!(&result, Err(StartError::Refused { reason, .. }) if reason == "launched"),
        "锁放了才拉：{result:?}"
    );
    assert!(started.elapsed() >= Duration::from_millis(600));
    assert_eq!(told.load(Ordering::Relaxed), 1);
    release.await.expect("没 panic");
}

/// 核心的锁一直在别人手里、一直连不上：到了上限是 `Timeout`，不拉第二个。
#[tokio::test]
async fn a_core_stuck_holding_the_lock_times_out() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let _lock = Lock::acquire(&root).expect("拿得到");
    let clock = Clock::new(patience(100, 400), QUIET);
    let never = || -> Command { panic!("锁在别人手里，不拉") };
    let result = start_with(&root, Starting::Asked, never, unreachable, &clock).await;
    assert!(matches!(result, Err(StartError::Timeout)), "{result:?}");
}

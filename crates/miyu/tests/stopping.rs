//! 停核心（施工 CS-6，`internal/blueprint/core.md`「停下」）：真的主程序、真的核心，Unix 上测。
//!
//! 收尾慢的造法：拉起一个一起来就 `hang` 的扩展（`miyu-test-extension`，不管标准输入），核心停它时要等 5 秒再杀。这 5 秒里看：
//! 第二次停的信号当场强退；收尾的总时限（测试口子 `MIYU_TEST_STOP_LIMIT_MS`）到了强退；不再接新的连接；这时敲的
//! `miyu core start` 等它走完，再拉起一个新的。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::crash_kept::Pids;
use crate::support::{Home, MIYU, count};
use crate::waiting::{Watched, signal};
use miyu_ipc::ConnectError;

/// `miyu` 旁边的测试程序 `name`：不在的说怎么编。
fn beside(name: &str) -> PathBuf {
    let path = Path::new(MIYU).with_file_name(name);
    assert!(
        path.is_file(),
        "{} 不在：cargo test --workspace 会编它",
        path.display()
    );
    path
}

/// 装一个一起来就 `hang` 的扩展，拉起核心（不空闲退出，`stop_limit` 是收尾总时限的测试口子），等扩展起来。交回核心和扩展、
/// 它的孙进程的进程号：丢掉时还在的照进程号收拾掉（施工 CS-7：断言没过、核心又留下了的，也不活过这一趟门禁）。
fn slow_to_stop(home: &Home, stop_limit: Option<u64>) -> (Child, Pids) {
    beside("miyu-test-extension");
    let record = home.dir.join("hang.pids");
    let package = home.root.packages().join("cs-hang");
    std::fs::create_dir_all(&package).expect("建得了");
    let manifest = format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Hang\" }}\n\n[command]\nname = \"cs-hang\"\nprogram = \"miyu-test-extension\"\nabout = {{ en = \"H\" }}\n\n[process]\nargs = [{:?}, \"hang\"]\nstart = \"always\"\n",
        format!("child:{}", record.display())
    );
    std::fs::write(package.join("package.toml"), manifest).expect("写得进");
    let mut command = home.core_staying();
    if let Some(limit) = stop_limit {
        command.env("MIYU_TEST_STOP_LIMIT_MS", limit.to_string());
    }
    let mut core = command.stdout(Stdio::piped()).spawn().expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    let pids = recorded(home, &record);
    (core, pids)
}

/// 等扩展往 `record` 写好两行进程号（最多 60 秒）。
fn recorded(home: &Home, record: &Path) -> Pids {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        let text = std::fs::read_to_string(record).unwrap_or_default();
        if let [one, two] = text.lines().map(str::trim).collect::<Vec<_>>().as_slice() {
            return Pids([(*one).to_string(), (*two).to_string()]);
        }
        assert!(Instant::now() < until, "扩展没起来：{}", home.core_log());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 等 `core` 退，最多 `wait`：交回退出码；到时还没退的杀掉、交回空的。
pub fn exit_within(core: &mut Child, wait: Duration) -> Option<ExitStatus> {
    let until = Instant::now() + wait;
    loop {
        if let Some(status) = core.try_wait().expect("问得了") {
            return Some(status);
        }
        if Instant::now() >= until {
            core.kill().expect("杀得了");
            core.wait().expect("收得了");
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 收尾时又来一次停的信号：当场强退，退出码 1，运行日志记一行（修之前第二次被吞掉，要等扩展的 5 秒）。
#[test]
fn a_second_stop_signal_ends_the_core_at_once() {
    let home = Home::new();
    let (mut core, _kept) = slow_to_stop(&home, None);
    let pid = core.id().to_string();
    signal(&pid, "TERM");
    std::thread::sleep(Duration::from_millis(300));
    let again = Instant::now();
    signal(&pid, "INT");
    let status = exit_within(&mut core, Duration::from_secs(15));
    let log = home.core_log();
    assert!(again.elapsed() < Duration::from_secs(2), "当场退：{log}");
    assert_eq!(status.and_then(|status| status.code()), Some(1), "{log}");
    assert_eq!(count(&log, "second stop signal, exiting now"), 1, "{log}");
    assert_eq!(count(&log, "stopped reason="), 0, "没走完收尾：{log}");
}

/// 收尾有总时限：到了就强退，退出码 1，运行日志记一行（修之前没有，卡住的收尾一直卡着）。
#[test]
fn stopping_has_a_time_limit() {
    let home = Home::new();
    let (mut core, _kept) = slow_to_stop(&home, Some(1000));
    let term = Instant::now();
    signal(&core.id().to_string(), "TERM");
    let status = exit_within(&mut core, Duration::from_secs(15));
    let log = home.core_log();
    let took = term.elapsed();
    assert!(
        (Duration::from_millis(1000)..Duration::from_millis(3000)).contains(&took),
        "{took:?}：{log}"
    );
    assert_eq!(status.and_then(|status| status.code()), Some(1), "{log}");
    assert_eq!(
        count(&log, "stopping took too long, exiting now"),
        1,
        "{log}"
    );
}

/// 收尾时不再接新连接：一开头就关掉监听、删掉 `run/socket`，头连是「没在跑」，不会连上了却没人回（修之前要等收尾做完）。
#[tokio::test]
async fn a_stopping_core_takes_no_new_connections() {
    let home = Home::new();
    let (mut core, _kept) = slow_to_stop(&home, None);
    signal(&core.id().to_string(), "TERM");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let reached = tokio::time::timeout(Duration::from_secs(3), miyu_ipc::connect(&home.root)).await;
    let alive = core.try_wait().expect("问得了").is_none();
    assert!(
        exit_within(&mut core, Duration::from_secs(30)).is_some(),
        "{}",
        home.core_log()
    );
    assert!(alive, "那时还在收尾");
    assert!(
        matches!(reached, Ok(Err(ConnectError::NotRunning))),
        "收尾时连是没在跑：{:?}",
        reached.map(|reached| reached.map(|_| ()))
    );
}

/// 核心在收尾时敲 `miyu core start`：等它走完、放了锁，再拉起一个新的，回来时新的在跑（修之前连上正在收尾的那一个就回了，
/// 回来不久它就没了）。
#[test]
fn starting_while_the_core_stops_brings_up_a_new_one() {
    let home = Home::new();
    let (mut old, _kept) = slow_to_stop(&home, None);
    signal(&old.id().to_string(), "TERM");
    std::thread::sleep(Duration::from_millis(300));
    let root = home.root.path();
    let mut command = crate::support::cli::command(root, root, "zh_CN.UTF-8", &["core", "start"]);
    command
        .env("MIYU_TEST_WAIT_SLOW_MS", "500")
        .env("MIYU_TEST_WAIT_LIMIT_MS", "30000")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut head = Watched::spawn(command);
    let started = head.exit_within(Duration::from_secs(40));
    let text = head.text();
    assert!(
        exit_within(&mut old, Duration::from_secs(30)).is_some(),
        "{}",
        home.core_log()
    );
    let (status, _) = started.unwrap_or_else(|| panic!("四十秒没回：{text}"));
    assert_eq!(status.code(), Some(0), "{text}");
    assert!(
        head.told("核心还在启动").is_some(),
        "等的时候说一声：{text}"
    );
    let log = home.core_log();
    assert_eq!(count(&log, "starting version="), 2, "拉起了新的：{log}");
    assert!(
        miyu_ipc::Lock::acquire(&home.root).is_err(),
        "回来时新的在跑：{log}"
    );
}

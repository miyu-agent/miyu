//! `shell` 只在 Unix 上测的几条：放到后台的、叫停、被信号杀掉、照命令里的词 `pkill -f`（从 `shell.rs` 拆出来，行数上限）。

use super::*;

/// 进程还在不在：`kill -0`。
fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("kill 起得来")
        .success()
}

/// 等进程 `pid` 没了，最多等两秒。
fn gone(pid: &str) -> bool {
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// 等场地里的 `name` 写好一个进程编号，最多等五秒。
fn pid(site: &Site, name: &str) -> String {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = std::fs::read_to_string(site.0.join("work").join(name))
            && text.ends_with('\n')
        {
            return text.trim().to_string();
        }
        assert!(Instant::now() < until, "{name} 没写出来");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[tokio::test]
async fn what_it_left_running_stops_when_it_ends() {
    let site = Site::new();
    let started = Instant::now();
    let done = run(&site, "sleep 30 & echo $! > bg.pid; echo started").await;
    assert!(started.elapsed() < Duration::from_secs(10), "不等后台的");
    assert_eq!(text(&done), "started\n");
    assert!(gone(&pid(&site, "bg.pid")), "放到后台的也停了");
}

/// 命令不在 shell 的命令行里（施工 4-8 再补）：照命令里的词 `pkill -f` 杀不到自己所在的 shell，命令也看不到放它的变量。
#[tokio::test]
async fn pkill_by_a_word_of_the_command_does_not_kill_its_own_shell() {
    let site = Site::new();
    let done = run(
        &site,
        "pkill -f miyu-4-8-self-match; echo survived; echo \"[${MIYU_COMMAND-unset}]\"",
    )
    .await;
    assert_eq!(text(&done), "survived\n[unset]\n");
}

#[tokio::test]
async fn interrupting_stops_the_whole_group() {
    let site = Site::new();
    let running = run(&site, "sleep 30 & echo $! > bg.pid; echo $$ > sh.pid; wait");
    let stopped = tokio::time::timeout(Duration::from_millis(800), running).await;
    assert!(stopped.is_err(), "还在跑的时候叫停");
    let (background, shell) = (pid(&site, "bg.pid"), pid(&site, "sh.pid"));
    assert!(gone(&background), "后台的停了");
    assert!(gone(&shell), "shell 自己也停了");
}

#[tokio::test]
async fn a_command_killed_by_a_signal_says_which() {
    let site = Site::new();
    let done = run(&site, "kill -9 $$").await;
    assert!(done.error);
    assert_eq!(text(&done), "Killed by signal 9\n");
    assert_eq!(done.human, Some(said("shell/signal").with("signal", "9")));
}

//! 核心有计划地停下，它拉起的长住的子进程起的子进程也跟着走（施工 CS-7，`internal/blueprint/core.md`「子进程随核心退出」
//! 第 5 条）：真的 `miyu core` 拉起扩展（一起来就 `hang`、再起一个睡着的孙进程）、命令型 MCP 服务（照 `crash_kept.rs` 的一段
//! `sh` 先在后台起 `sleep`），记下两个进程号；发 `SIGTERM` 让核心照「停下」走完（退出码 0、记 `stopped reason=signal`），
//! 两个都要在几秒内没了。
//!
//! 修之前：扩展等 5 秒没退被杀、MCP 服务读到标准输入关了自己退，组里留下的孙进程没人管；最后一个拿着组的丢了，只收掉看门的，
//! 孙进程过继给系统，核心走了以后一直留着（门禁每趟留 5 个 `miyu-test-extension hang`，来自 `stopping.rs`）。只在 Unix 上测：
//! Windows 上核心退出时作业对象关掉，里面的都结束（`crash_kept.rs` 测了扩展那一条）。

use std::process::Child;
use std::time::{Duration, Instant};

use crate::crash_kept::{Pids, alive, beside, gone, pids, start, wrapper};
use crate::support::{Home, count};
use crate::waiting::signal;

/// 发 `SIGTERM`，等核心照「停下」走完（最多 30 秒，扩展要等 5 秒）；两个进程号都要没了。
fn stop_and_check(home: &Home, mut core: Child, pids: &Pids, what: &str) {
    signal(&core.id().to_string(), "TERM");
    let until = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = core.try_wait().expect("问得了") {
            break Some(status);
        }
        if Instant::now() >= until {
            core.kill().expect("杀得了");
            core.wait().expect("收得了");
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let log = home.core_log();
    assert_eq!(status.and_then(|status| status.code()), Some(0), "{log}");
    assert_eq!(count(&log, "stopped reason=signal"), 1, "走完了收尾：{log}");
    let left: Vec<&String> = match gone(pids) {
        true => Vec::new(),
        false => pids.0.iter().filter(|pid| alive(pid)).collect(),
    };
    assert!(
        left.is_empty(),
        "核心停了，{what}（{:?}）还剩 {left:?}",
        pids.0
    );
}

#[tokio::test]
async fn an_extensions_child_goes_when_the_core_stops() {
    let home = Home::new();
    beside("miyu-test-extension");
    let record = home.dir.join("extension.pids");
    let package = home.root.packages().join("cs-kept");
    std::fs::create_dir_all(&package).expect("建得了");
    let manifest = format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Kept\" }}\n\n[command]\nname = \"cs-kept\"\nprogram = \"miyu-test-extension\"\nabout = {{ en = \"K\" }}\n\n[process]\nargs = [{:?}, \"hang\"]\nstart = \"always\"\n",
        format!("child:{}", record.display())
    );
    std::fs::write(package.join("package.toml"), manifest).expect("写得进");
    let core = start(&home);
    let pids = pids(&home, &record).await;
    stop_and_check(&home, core, &pids, "扩展");
}

#[tokio::test]
async fn an_mcp_servers_child_goes_when_the_core_stops() {
    let home = Home::new();
    let record = home.dir.join("mcp.pids");
    let script = wrapper(&record, &beside("miyu-test-mcp"));
    home.system_config(&format!(
        "[mcp.kept]\ncommand = [\"/bin/sh\", \"-c\", {script:?}]\ntimeout_ms = 1000\n"
    ));
    let core = start(&home);
    let pids = pids(&home, &record).await;
    stop_and_check(&home, core, &pids, "MCP 服务");
}

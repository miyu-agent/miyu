//! 前台到时挪到后台（施工 7-12，`docs/blueprint/tools/shell.md`「怎么走」第 6 条）：会话里有任务端口的，前台的命令到了
//! 超时不杀，交给任务表接着跑，调用当场交回编号；跑完了照后台命令记 `job.reported`，输出是挪过去以前读到的接上以后的，
//! 闲着的她被叫醒。开头是 `sleep` 的照旧杀。真的 `shell`，三个平台都跑。

use miyu_kernel::event::{Body, Effect, JobReason, ToolStatus};
use miyu_kernel::id::JobId;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;

use crate::support::jobs::*;
use crate::support::*;

/// 前台跑 `command`，超时 `timeout` 毫秒。
fn foreground(command: &str, timeout: u64) -> String {
    serde_json::json!({ "command": command, "description": "Build", "timeout": timeout })
        .to_string()
}

#[tokio::test]
async fn a_slow_foreground_command_moves_to_the_background() {
    let home = Home::outside_temp();
    let command = if cfg!(windows) {
        "Write-Output before; Start-Sleep -Milliseconds 1900; Write-Output after; exit 3"
    } else {
        // 挪过去以后的 `\r\n` 照后台命令的规矩换成 `\n`。
        "echo before; sleep 1.9; printf 'after\\r\\n'; exit 3"
    };
    let args = foreground(command, 500);
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("在后台跑着。"),
        Play::Says("看到了。"),
    ]);
    let handle = home.create_as(&script, &base_system(), full(&home)).await;
    talk(&handle, "cmd-1", "编一下").await;
    assert!(home.jobs.running(), "这一轮结束了，命令还在跑");
    let log = until_logged(&home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            == 2
    })
    .await;
    let moved = results(&log)[0];
    assert_eq!(moved.status, ToolStatus::Ok, "{moved:?}");
    let text = format!("{:?}", moved.blocks);
    assert!(text.contains("Started j1 in the background."), "{text}");
    assert!(
        moved.effects.iter().any(|effect| matches!(effect, Effect::JobStarted(job) if job.job == JobId::new(1).unwrap() && job.title == "Build")),
        "{moved:?}"
    );
    let [(_, reported)] = reports(&log)[..] else {
        panic!("记了一条结束：{log:#?}")
    };
    assert_eq!(reported.reason, JobReason::Exited);
    assert_eq!(reported.exit_code, Some(3));
    let output = reported.output.clone().expect("存了输出");
    let blobs = Blobs::new(home.root.blobs());
    let text = String::from_utf8(blobs.get(&output).expect("blob 在")).expect("是 UTF-8");
    assert_eq!(text, "before\nafter\n", "挪过去以前读到的接上以后的");
    until_idle(&home).await;
}

#[tokio::test]
async fn a_sleep_is_still_stopped() {
    let home = Home::outside_temp();
    // PowerShell 里 `sleep` 是 `Start-Sleep` 的别名：同一句三个平台都能跑。两秒以内的短停照跑（更长的前台干等不跑，2026-10-26）。
    let args = foreground("sleep 1.5", 300);
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("停了。"),
    ]);
    let handle = home.create_as(&script, &base_system(), full(&home)).await;
    talk(&handle, "cmd-1", "等一下").await;
    let log = home.log(handle.id());
    let stopped = results(&log)[0];
    assert_eq!(stopped.status, ToolStatus::Error, "{stopped:?}");
    assert!(
        format!("{:?}", stopped.blocks).contains("Stopped after 300 ms"),
        "{stopped:?}"
    );
    assert!(reports(&log).is_empty());
    assert!(!home.jobs.running(), "杀掉了，任务表里没有");
}

#[tokio::test]
async fn a_moved_command_can_be_stopped() {
    let home = Home::outside_temp();
    let command = if cfg!(windows) {
        "Write-Output started; & { Start-Sleep -Seconds 30 }"
    } else {
        "echo started; sh -c 'sleep 30'"
    };
    let args = foreground(command, 300);
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("在后台跑着。"),
        Play::Says("停了。"),
    ]);
    let handle = home.create_as(&script, &base_system(), full(&home)).await;
    talk(&handle, "cmd-1", "跑一下").await;
    let job = JobId::new(1).expect("合写法");
    let stopped = within("人停", handle.stop_job(job, alice(), id("stop-1")))
        .await
        .expect("会话在跑");
    assert_eq!(stopped, Ok(()));
    let log = until_logged(&home, handle.id(), |log| !reports(log).is_empty()).await;
    let [(_, reported)] = reports(&log)[..] else {
        panic!("记了一条结束：{log:#?}")
    };
    assert_eq!(reported.reason, JobReason::Stopped);
    until_idle(&home).await;
}

#[tokio::test]
async fn a_long_output_before_the_move_is_cut_in_the_middle() {
    let home = Home::outside_temp();
    // Windows 上 PowerShell 起得慢：一次写完，超时也放宽，挪过去之前一定读到了超过 30000 个字。等的那一段包进 `& { }`：顶层
    // 不短于两秒的前台干等不跑（施工 4-8 补三）。
    let (command, timeout) = if cfg!(windows) {
        (
            "Write-Output ((1..20000) -join \"`n\"); & { Start-Sleep -Milliseconds 6000 }; Write-Output after",
            4000,
        )
    } else {
        ("seq 1 20000; sleep 1.5; echo after", 800)
    };
    let args = foreground(command, timeout);
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("在后台跑着。"),
        Play::Says("看到了。"),
    ]);
    let handle = home.create_as(&script, &base_system(), full(&home)).await;
    talk(&handle, "cmd-1", "数一下").await;
    let log = until_logged(&home, handle.id(), |log| !reports(log).is_empty()).await;
    let [(_, reported)] = reports(&log)[..] else {
        panic!("记了一条结束：{log:#?}")
    };
    let output = reported.output.clone().expect("存了输出");
    let blobs = Blobs::new(home.root.blobs());
    let text = String::from_utf8(blobs.get(&output).expect("blob 在")).expect("是 UTF-8");
    assert!(text.starts_with("1\n2\n3\n"), "开头在");
    assert!(
        text.contains("characters omitted ...]\n"),
        "中间省了，照给她看的说"
    );
    assert!(
        text.ends_with("19999\n20000\nafter\n"),
        "结尾接上挪过去以后的"
    );
    until_idle(&home).await;
}

/// 挪过去的命令结束了，组里它放出去的也杀掉，和前台、后台一样（Unix 都杀，Windows 上退出以后不按编号杀；看 `/proc`，
/// 只在 Linux 上查）。
#[cfg(target_os = "linux")]
#[tokio::test]
async fn what_a_moved_command_left_running_is_killed_when_it_ends() {
    let home = Home::outside_temp();
    let pid = home.scratch.0.join("left.pid");
    // 开头不能是 `sleep`：那样的照旧杀，不挪。
    let command = format!(
        "echo started; sleep 300 & echo $! > '{}'; sleep 1",
        pid.display()
    );
    let args = foreground(&command, 300);
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("在后台跑着。"),
        Play::Says("看到了。"),
    ]);
    let handle = home.create_as(&script, &base_system(), full(&home)).await;
    talk(&handle, "cmd-1", "跑一下").await;
    until_logged(&home, handle.id(), |log| !reports(log).is_empty()).await;
    let left = std::fs::read_to_string(&pid).expect("记下了放出去的那一个");
    let proc = std::path::PathBuf::from(format!("/proc/{}", left.trim()));
    // 它自己要跑 300 秒：10 秒内没了，才是结束时杀的。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while proc.exists() && !zombie(&proc) {
        assert!(std::time::Instant::now() < deadline, "放出去的还在跑");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    until_idle(&home).await;
}

/// 已经死了、等着被收的也算没了。
#[cfg(target_os = "linux")]
fn zombie(proc: &std::path::Path) -> bool {
    std::fs::read_to_string(proc.join("stat")).is_ok_and(|stat| {
        stat.rsplit(')')
            .next()
            .is_some_and(|rest| rest.trim_start().starts_with('Z'))
    })
}

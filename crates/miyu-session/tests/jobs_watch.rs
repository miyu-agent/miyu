//! 监视的后台命令（施工 MN-1 上，设计 56，`docs/blueprint/agents.md` 第四条）：真的 `shell` 写了 `watch`，三个平台都跑。
//! 一下子出的三行合成一批、记一条 `job.lines`、叫醒她一次；标准错误不叫醒、只在 `jobs/<编号>.out` 里；结束了照常报。

use miyu_kernel::event::{Body, Event, JobLines, JobReason};
use miyu_session::testkit::{Play, Script};

use crate::support::jobs::*;
use crate::support::*;

/// 日志里的 `job.lines`，照先后。
fn watched(log: &[Event]) -> Vec<(&Event, &JobLines)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::JobLines(lines) => Some((event, lines)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn three_lines_wake_her_once_and_stderr_does_not() {
    let home = Home::outside_temp();
    let command = if cfg!(windows) {
        "Write-Output one; Write-Output two; Write-Output three; [Console]::Error.WriteLine('oops'); Start-Sleep -Milliseconds 1500"
    } else {
        "printf 'one\\ntwo\\nthree\\n'; echo oops >&2; sleep 1.5"
    };
    let args = serde_json::json!({ "command": command, "description": "Watch it", "watch": true })
        .to_string();
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("盯着了。"),
        Play::Says("看到那三行了。"),
        Play::Says("结束了。"),
    ]);
    let handle = home.create_as(&script, &base_system(), full(&home)).await;
    talk(&handle, "cmd-1", "盯一下").await;
    let log = until_logged(&home, handle.id(), |log| {
        !reports(log).is_empty()
            && log
                .iter()
                .filter(|event| matches!(event.body, Body::TurnEnded(_)))
                .count()
                == 3
    })
    .await;
    let started = results(&log)[0];
    let said = started
        .blocks
        .iter()
        .map(|block| format!("{block:?}"))
        .collect::<String>();
    assert!(said.contains("Each stdout line wakes you"), "{said}");
    let lines = watched(&log);
    let [(event, lines)] = lines[..] else {
        panic!("一下子出的三行合成一批：{log:#?}")
    };
    assert_eq!((lines.text.as_str(), lines.lines), ("one\ntwo\nthree", 3));
    assert!(!lines.text.contains("oops"), "标准错误不叫醒");
    assert_eq!(event.turn, None, "不带回合编号");
    let opened = log.iter().find_map(|later| match &later.body {
        Body::TurnStarted(started) if started.trigger == Some(event.seq) => Some(later.seq),
        _ => None,
    });
    assert!(opened.is_some(), "由它开了一轮：{log:#?}");
    let [(_, reported)] = reports(&log)[..] else {
        panic!("结束报了一条")
    };
    assert_eq!(reported.reason, JobReason::Exited);
    let file = miyu_store::jobs::output_path(&home.root.session_dir(handle.id()), &reported.job);
    let text = std::fs::read_to_string(file).expect("输出文件在");
    assert!(
        text.contains("three") && text.contains("oops"),
        "两路都进文件：{text:?}"
    );
}

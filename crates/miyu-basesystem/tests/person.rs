//! 给人跑一条命令（施工 SH-1 上，设计 57，`docs/blueprint/tools/shell.md`「给人跑」）：真的起进程，三个平台都跑。交回结构化的
//! 结果；人打的 `sleep` 照跑，她的同一句照旧被拒；叫停的整组杀、交回到这时的输出；不跑命令的工具没有这个钩子。

use std::time::Duration;

use serde_json::json;

use miyu_tool::{Halt, PersonCommand, PersonEnd, PersonRun};

use crate::support::{Site, tool};

/// 这台机器上的写法。
fn script<'a>(unix: &'a str, windows: &'a str) -> &'a str {
    if cfg!(windows) { windows } else { unix }
}

/// 在临时目录里替人跑 `command`，`halt` 照给的。
async fn run(command: &str, halt: Halt) -> PersonRun {
    let shell = tool("shell");
    let running = shell
        .run_for_person(
            PersonCommand {
                command: command.to_string(),
                cwd: std::env::temp_dir(),
            },
            halt,
        )
        .expect("shell 有这个钩子");
    tokio::time::timeout(Duration::from_secs(60), running)
        .await
        .expect("一分钟内跑完")
        .expect("起得来")
}

#[tokio::test]
async fn a_command_comes_back_as_its_exit_and_output() {
    let ran = run(
        script(
            "echo hi; echo oops >&2; exit 3",
            "Write-Output hi; [Console]::Error.WriteLine('oops'); exit 3",
        ),
        Halt::new(),
    )
    .await;
    assert_eq!(ran.ended, PersonEnd::Exited);
    assert_eq!((ran.exit, ran.signal), (Some(3), None));
    assert!(
        ran.text.contains("hi\n") && ran.text.contains("oops\n"),
        "{:?}",
        ran.text
    );
    assert_eq!(ran.omitted, 0);
}

#[tokio::test]
async fn a_person_may_sleep_where_she_may_not() {
    let sleep = script("sleep 2", "Start-Sleep -Seconds 2");
    let ran = run(sleep, Halt::new()).await;
    assert_eq!(
        (ran.ended, ran.exit),
        (PersonEnd::Exited, Some(0)),
        "人打的照跑"
    );
    let site = Site::new();
    let done = site
        .done("shell", json!({ "command": sleep, "description": "Wait" }))
        .await;
    assert!(done.error, "她的照旧拒");
    assert_eq!(
        done.human.map(|said| said.key),
        Some("software/basesystem/shell/no-sleep".to_string())
    );
}

#[tokio::test]
async fn halting_kills_it_and_keeps_what_it_printed() {
    let halt = Halt::new();
    let later = halt.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        later.halt();
    });
    let ran = run(
        script(
            "echo first; sleep 30",
            "Write-Output first; Start-Sleep -Seconds 30",
        ),
        halt,
    )
    .await;
    assert_eq!(ran.ended, PersonEnd::Interrupted);
    assert_eq!((ran.exit, ran.signal), (None, None));
    assert!(ran.text.contains("first"), "{:?}", ran.text);
    assert!(ran.duration_ms < 30_000, "没等它睡完");
}

#[tokio::test]
async fn a_tool_that_runs_no_commands_has_no_hook() {
    let read = tool("read");
    let none = read.run_for_person(
        PersonCommand {
            command: "echo hi".to_string(),
            cwd: std::env::temp_dir(),
        },
        Halt::new(),
    );
    assert!(none.is_none());
}

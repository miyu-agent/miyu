//! 前台干等的认法（2026-10-26）：顶层的、不短于两秒的 `sleep` 拒；管道、循环体、后台、短停、引号里的照跑。

use super::waits;

#[test]
fn a_foreground_wait_is_caught() {
    for command in [
        "sleep 30",
        "sleep 2",
        "sleep 1m",
        "sleep 1 1",
        "sleep 0.5m",
        "sleep $DELAY",
        "  sleep 5  ",
        "echo start; sleep 10",
        "sleep 10 && cat out.log",
        "make || sleep 5",
        "cd /tmp\nsleep 3",
        "sleep 30 & sleep 30",
        "Start-Sleep -Seconds 20",
        "Start-Sleep 5",
        "start-sleep -Milliseconds 3000",
    ] {
        assert!(waits(command), "{command}");
    }
}

#[test]
fn other_commands_run() {
    for command in [
        "sleep 1",
        "sleep 0.5; echo ok",
        "sleep 30 &",
        "sleep 30 & echo $! > pid",
        "while true; do sleep 5; done",
        "for i in 1 2; do sleep 3; done",
        "until test -f done; do sleep 1; done",
        "echo 'sleep 30'",
        "echo \"a; sleep 30\"",
        "(sleep 30; echo late) &",
        "yes | sleep 3",
        "sh -c 'sleep 20'",
        "timeout 5 sleep 30",
        "Start-Sleep -Milliseconds 500",
        "sleeper 30",
        "",
    ] {
        assert!(!waits(command), "{command}");
    }
}

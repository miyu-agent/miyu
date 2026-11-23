//! 看门的小程序本身（施工 LW-28）：不带参数的读到标准输入的结尾就杀掉自己所在的整个组（读到的东西不管）；带了参数的不看门，
//! 说一句用法、退出 125。每次都起在自己的组里：认错了真去看门，也只杀它自己。Windows 上不看门，什么参数都退出 125。

use std::process::{Command, Stdio};

/// cargo 编出来的看门的。
const WATCHDOG: &str = env!("CARGO_BIN_EXE_miyu-watchdog");

/// 起在自己的组里（Unix）。
#[cfg(unix)]
fn alone() -> Command {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(WATCHDOG);
    command.process_group(0);
    command
}

/// 同上：Windows 上没有进程组，直接起。
#[cfg(not(unix))]
fn alone() -> Command {
    Command::new(WATCHDOG)
}

#[cfg(unix)]
#[test]
fn it_kills_its_group_at_the_end_of_stdin() {
    use std::io::Write;
    use std::os::unix::process::ExitStatusExt;
    // 标准输入是空的：马上读到结尾。
    let out = alone().stdin(Stdio::null()).output().expect("起得来");
    assert_eq!(out.status.signal(), Some(9), "{out:?}");
    assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
    // 先读到些字，再读到结尾：读到的不管，照样杀。
    let mut child = alone()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("起得来");
    let mut input = child.stdin.take().expect("接了管道");
    input.write_all(&[b'x'; 200]).expect("写得进");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        child.try_wait().expect("问得了").is_none(),
        "还没读到结尾，等着"
    );
    drop(input);
    assert_eq!(child.wait().expect("等得到").signal(), Some(9));
}

#[test]
fn arguments_are_a_usage_error() {
    for args in [&["lifeline"][..], &["x", "y"]] {
        let out = alone()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("起得来");
        assert_eq!(out.status.code(), Some(125), "{args:?}：{out:?}");
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(said.starts_with("miyu-watchdog: usage"), "{said}");
    }
}

#[cfg(not(unix))]
#[test]
fn it_does_not_watch_off_unix() {
    let out = alone().stdin(Stdio::null()).output().expect("起得来");
    assert_eq!(out.status.code(), Some(125), "{out:?}");
}

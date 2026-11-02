use std::process::Command;

use super::*;

/// 自己在跑。
#[test]
fn this_process_is_running() {
    assert!(process_running(std::process::id()));
}

/// 起一个马上退出的子进程：等它退了，进程号就不算在跑。`Child` 还拿着它的句柄，系统里那个进程对象还在，看的是退出码。
#[test]
fn an_exited_child_is_not_running() {
    let mut child = Command::new("cmd")
        .args(["/c", "exit 0"])
        .spawn()
        .expect("起得来");
    let pid = child.id();
    child.wait().expect("等得到");
    assert!(!process_running(pid));
    drop(child);
    assert!(!process_running(pid), "句柄关了以后也不在");
}

/// 系统里没有的进程号（进程号是 4 的倍数，这个不会是）：不在。
#[test]
fn a_pid_nobody_has_is_not_running() {
    assert!(!process_running(u32::MAX - 2));
}

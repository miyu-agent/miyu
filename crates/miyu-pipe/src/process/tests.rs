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

/// 子进程起得不比自己早，自己起得不比子进程晚（施工 XG-1补2）：认子孙时就照这个先后认出被重用的父进程号。系统里没有的进程号
/// 问不出。
#[test]
fn a_child_is_created_no_earlier_than_this_process() {
    let mut child = Command::new("cmd")
        .args(["/c", "ping -n 3 127.0.0.1 >NUL"])
        .spawn()
        .expect("起得来");
    let child_created = process_created(child.id());
    let own = process_created(std::process::id()).expect("问得出自己的");
    child.kill().expect("杀得掉");
    child.wait().expect("等得到");
    let child_created = child_created.expect("问得出子进程的");
    assert!(own <= child_created, "自己 {own}，子进程 {child_created}");
    assert!(own > 0);
    assert_eq!(process_created(u32::MAX - 2), None);
}

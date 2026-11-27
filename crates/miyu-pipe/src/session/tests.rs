use std::os::unix::process::CommandExt;
use std::process::Command;

use super::*;

/// 开了新会话的子进程：会话号、进程组号都是它自己的进程号，不是测试进程的。
#[test]
fn the_child_leads_its_own_session_and_group() {
    let mut command = Command::new("python3");
    command.args([
        "-c",
        "import os; print(os.getsid(0), os.getpgrp(), os.getpid())",
    ]);
    new_session(&mut command);
    let out = command.output().expect("起得来 python3");
    assert!(out.status.success(), "{out:?}");
    let said = String::from_utf8(out.stdout).expect("是 UTF-8");
    let ids: Vec<i32> = said
        .split_whitespace()
        .map(|id| id.parse().expect("是数字"))
        .collect();
    let [sid, group, pid] = ids[..] else {
        panic!("三个数：{said}");
    };
    assert_eq!((sid, group), (pid, pid), "{said}");
    let mine = rustix::process::getsid(None).expect("问得到");
    assert_ne!(
        sid,
        rustix::process::Pid::as_raw(Some(mine)),
        "不在测试进程的会话里"
    );
}

/// 同时设了 `process_group` 的：先当了组长，`setsid()` 不成，起的时候就报错（见 `session.rs` 开头）。
#[test]
fn a_process_group_as_well_keeps_it_from_starting() {
    let mut command = Command::new("true");
    command.process_group(0);
    new_session(&mut command);
    let error = command.spawn().expect_err("起不来");
    assert_eq!(
        error.raw_os_error(),
        Some(rustix::io::Errno::PERM.raw_os_error()),
        "{error}"
    );
}

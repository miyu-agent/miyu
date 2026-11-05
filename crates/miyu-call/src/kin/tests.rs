//! 认人（施工 XG-1 下）：父进程链照假的表找、真的进程照这台机器问；两种认法各自只认自己的人。

use std::collections::BTreeMap;

use super::*;

/// 照一张假的表问父进程。
fn table(pairs: &[(u32, u32)]) -> impl Fn(u32) -> Option<u32> {
    let parents: BTreeMap<u32, u32> = pairs.iter().copied().collect();
    move |pid| parents.get(&pid).copied()
}

#[test]
fn a_child_or_grandchild_descends_and_others_do_not() {
    // 1 号下面是扩展 100，100 下面是 shell 200，200 下面是 `miyu call` 300；400 是别人家的。
    let parent = table(&[(100, 1), (200, 100), (300, 200), (400, 1), (500, 400)]);
    assert!(descends(200, 100, &parent));
    assert!(descends(300, 100, &parent));
    assert!(!descends(500, 100, &parent));
    assert!(!descends(400, 100, &parent));
    // 扩展进程自己不算子孙。
    assert!(!descends(100, 100, &parent));
    // 问不出父进程的不是。
    assert!(!descends(999, 100, &parent));
}

#[test]
fn a_loop_or_a_chain_too_long_stops() {
    // 绕回来的（Windows 的旧编号会这样）、父进程是自己的、0 号：停下，不是。
    assert!(!descends(10, 100, table(&[(10, 11), (11, 10)])));
    assert!(!descends(10, 100, table(&[(10, 10)])));
    assert!(!descends(10, 100, table(&[(10, 0)])));
    // 一条比上限还长的链：到不了。
    let long: Vec<(u32, u32)> = (1..=1000).map(|pid| (pid, pid + 1)).collect();
    assert!(!descends(1, 1001, table(&long)));
    assert!(descends(1, 200, table(&long)));
}

#[test]
fn the_parent_in_proc_stat_comes_after_the_last_parenthesis() {
    assert_eq!(linux_parent("300 (miyu) S 200 300 200 0 -1"), Some(200));
    // 程序名里有空格、括号的。
    assert_eq!(linux_parent("300 (a) b (c) R 42 1 1"), Some(42));
    assert_eq!(linux_parent("300 (miyu)"), None);
    assert_eq!(linux_parent("not a stat line"), None);
}

/// 起一个一会儿就走的子进程。
fn child() -> std::process::Child {
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/c", "ping -n 5 127.0.0.1 >NUL"]);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("sleep");
        command.arg("5");
        command
    };
    command.spawn().expect("起得来")
}

/// 收掉子进程：已经走了的不管。
#[expect(clippy::let_underscore_must_use, reason = "它可能已经自己走了")]
fn stop(mut child: std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// 照这台机器问：子进程是测试进程的子孙，测试进程不是子进程的。
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[tokio::test]
async fn a_real_child_descends_from_this_process() {
    let child = child();
    let (me, it) = (std::process::id(), child.id());
    let ours = Kin::descendant_of(me).ours(Some(it)).await;
    let reversed = Kin::descendant_of(it).ours(Some(me)).await;
    let itself = Kin::descendant_of(me).ours(Some(me)).await;
    let unknown = Kin::descendant_of(me).ours(None).await;
    stop(child);
    assert!(ours);
    assert!(!reversed);
    assert!(!itself);
    assert!(!unknown);
}

/// `shell` 的那一种：记下了的组照它认，扩展的那一种没有记的那一头。
#[cfg(unix)]
#[tokio::test]
async fn the_command_kind_checks_the_bound_group() {
    let kin = Kin::command();
    let own = rustix::process::getpgid(None).expect("查得到");
    let own = u32::try_from(own.as_raw_nonzero().get()).unwrap();
    kin.binder().expect("命令的有").bind(own);
    assert!(kin.ours(Some(std::process::id())).await);
    let other = Kin::command();
    other.binder().expect("命令的有").bind(own + 1);
    assert!(!other.ours(Some(std::process::id())).await);
    assert!(Kin::descendant_of(1).binder().is_none());
}

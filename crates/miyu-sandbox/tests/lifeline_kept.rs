//! 长住的子进程共用一个看门的（施工 LW-28，`internal/blueprint/core.md`「子进程随核心退出」第 5 条，`lifeline/unix/shared.rs`）：
//! 一根生命线上 `keep` 几次都进同一个组、同一个看门的，生命线断了全组连孙进程都没了；共用的看门的被杀了，下一次 `keep` 再起一个
//! 进同一个组，原来组里的照样被收掉；组里已经一个进程都没有了的另起一组；丢掉一个拿着的不收看门的，丢掉最后一个才收掉它、
//! 不留僵尸进程。`miyu-watchdog`、退回 `/bin/sh` 两种看门的都测。

#![cfg(unix)]

mod support;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use miyu_sandbox::lifeline::{Kept, Lifeline};
use support::family::{Running, alive, family_command, gone, group_of, kill, killed, run, running};

/// 两种看门的：`miyu-watchdog`、退回 `/bin/sh`（没给）。
fn both() -> [Option<PathBuf>; 2] {
    [Some(support::watchdog()), None]
}

/// 照 `keep` 起一条带孙进程的 `sh`：交回拿着的组和它。
fn kept(lifeline: &Lifeline, watchdog: Option<&std::path::Path>) -> (Kept, Running) {
    let mut command = family_command();
    let kept = lifeline.keep(watchdog, &mut command).expect("起得来看门的");
    (kept, run(&mut command))
}

/// `kept` 的组号、看门的进程号，写成字。
fn ids(kept: &Kept) -> (String, String) {
    (
        kept.group().expect("有组").to_string(),
        kept.watchdog().expect("有看门的").to_string(),
    )
}

/// 让 `child` 退出、收掉：先杀孙进程、等它被 `sh` 收掉，`sh` 跟着做完退出。组里不留它们的僵尸进程（核心收子进程也是这样）。
fn end(child: &mut Running) {
    kill(&child.grandchild);
    assert!(gone(&child.grandchild), "孙进程收掉了");
    child.command.wait().expect("等得到");
}

/// 杀掉 `pid`，等它变成僵尸或者没了（它是这个测试进程的子进程，收它的是 `Kept` 里的看门的）。
fn kill_watchdog(pid: &str) {
    kill(pid);
    let deadline = Instant::now() + Duration::from_secs(5);
    while alive(pid) && !support::family::zombie(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn long_lived_children_share_one_watchdog_and_die_together() {
    for given in both() {
        let lifeline = Lifeline::new().expect("建得起管道");
        let started: Vec<(Kept, Running)> =
            (0..3).map(|_| kept(&lifeline, given.as_deref())).collect();
        let (group, watchdog) = ids(&started[0].0);
        for (one, child) in &started {
            assert_eq!(ids(one), (group.clone(), watchdog.clone()), "{given:?}");
            assert_eq!(group_of(&child.pid), group, "子进程在共用的组里");
            assert_eq!(group_of(&child.grandchild), group, "孙进程也在");
        }
        assert_eq!(group, watchdog, "看门的是组长");
        std::thread::sleep(Duration::from_millis(200));
        let (mut kepts, mut children): (Vec<Kept>, Vec<Running>) = started.into_iter().unzip();
        assert!(children.iter_mut().all(running), "生命线还在，照常跑");
        drop(lifeline);
        for child in &mut children {
            assert!(killed(child), "整组被 SIGKILL，孙进程也没了：{given:?}");
        }
        kepts.clear();
        assert!(gone(&watchdog), "看门的收掉了");
    }
}

#[test]
fn a_killed_shared_watchdog_is_replaced_in_the_same_group() {
    for given in both() {
        let lifeline = Lifeline::new().expect("建得起管道");
        let (first_kept, mut first) = kept(&lifeline, given.as_deref());
        let (group, watchdog) = ids(&first_kept);
        kill_watchdog(&watchdog);
        assert!(running(&mut first), "看门的没了，子进程照常跑");
        let (second_kept, mut second) = kept(&lifeline, given.as_deref());
        let (again, replaced) = ids(&second_kept);
        assert_eq!(again, group, "还是同一个组：{given:?}");
        assert_ne!(replaced, watchdog, "换了一个看门的");
        assert_eq!(group_of(&replaced), group, "新看门的进了这个组");
        assert_eq!(group_of(&second.pid), group);
        assert_eq!(
            first_kept.watchdog(),
            second_kept.watchdog(),
            "拿着的都认新的"
        );
        drop(lifeline);
        assert!(killed(&mut first), "原来组里的照样被收掉：{given:?}");
        assert!(killed(&mut second));
        drop((first_kept, second_kept));
        assert!(gone(&replaced));
    }
}

#[test]
fn an_empty_group_is_not_reused() {
    for given in both() {
        let lifeline = Lifeline::new().expect("建得起管道");
        let (first_kept, mut first) = kept(&lifeline, given.as_deref());
        let (group, watchdog) = ids(&first_kept);
        kill_watchdog(&watchdog);
        // 组里唯一的子进程也退了、收掉了，拿着的还没丢：组里一个进程都没有了（看门的僵尸在下一次 `keep` 时收掉）。
        end(&mut first);
        let (second_kept, mut second) = kept(&lifeline, given.as_deref());
        let (other, _) = ids(&second_kept);
        assert_ne!(other, group, "另起了一组：{given:?}");
        assert_eq!(group_of(&second.pid), other);
        drop(lifeline);
        assert!(killed(&mut second), "新组照样跟着生命线走：{given:?}");
        drop((first_kept, second_kept));
    }
}

#[test]
fn the_watchdog_stays_until_the_last_holder_is_gone() {
    for given in both() {
        let lifeline = Lifeline::new().expect("建得起管道");
        let (first_kept, mut first) = kept(&lifeline, given.as_deref());
        let (second_kept, mut second) = kept(&lifeline, given.as_deref());
        let (group, watchdog) = ids(&first_kept);
        // 头一个退出、收掉、丢下：看门的还在，第二个照样有人看着。
        end(&mut first);
        drop(first_kept);
        assert!(
            alive(&watchdog) && !support::family::zombie(&watchdog),
            "{given:?}"
        );
        // 第二个也退了、丢下：看门的杀掉、收掉，不留僵尸。
        end(&mut second);
        drop(second_kept);
        assert!(!alive(&watchdog), "看门的收掉了，不是僵尸：{given:?}");
        // 再 `keep` 的另起一组、一个新看门的。
        let (third_kept, mut third) = kept(&lifeline, given.as_deref());
        assert_ne!(ids(&third_kept).0, group);
        drop(lifeline);
        assert!(killed(&mut third));
        drop(third_kept);
    }
}

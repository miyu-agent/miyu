//! 读 CPU：`cpu.stat`、`/proc/<进程号>/cgroup` 的读法；Linux 上读得到自己。

use super::*;

#[test]
fn usage_is_read_from_cpu_stat() {
    let text = "usage_usec 1234567\nuser_usec 1000000\nsystem_usec 234567\nnr_periods 0\n";
    assert_eq!(usage(text), Some(1234.567));
    assert_eq!(usage("user_usec 5\n"), None);
    assert_eq!(usage("usage_usec many\n"), None);
}

#[test]
fn the_unified_line_is_found() {
    assert_eq!(
        unified("0::/user.slice/user-1000.slice/run-r1.scope\n"),
        Some("/user.slice/user-1000.slice/run-r1.scope")
    );
    assert_eq!(unified("12:cpu,cpuacct:/a\n0::/b\n"), Some("/b"));
    assert_eq!(unified("12:cpu,cpuacct:/a\n"), None);
}

#[test]
fn since_needs_both_ends() {
    let before = Cpu {
        threads: Some(10.0),
        group: None,
    };
    let after = Cpu {
        threads: Some(12.5),
        group: Some(40.0),
    };
    assert_eq!(
        after.since(&before),
        Cpu {
            threads: Some(2.5),
            group: None,
        }
    );
}

#[cfg(target_os = "linux")]
#[test]
fn this_process_is_read_on_linux() {
    let probe = Probe {
        pid: std::process::id(),
        group: group_of(std::process::id()),
    };
    let first = probe.sample();
    let mut x: u64 = 0;
    for n in 0..2_000_000u64 {
        x = x.wrapping_mul(31).wrapping_add(n);
    }
    assert_ne!(x, 1);
    let spent = probe.sample().since(&first);
    assert!(spent.threads.is_some_and(|ms| ms >= 0.0));
}

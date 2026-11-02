//! 清留下的目录（施工 WC-1）：在真的临时目录里摆几样，问进程在不在用假的，三个平台一样测。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-leftovers-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }

    /// 摆好：几个名字对得上的目录（`1`、`2` 里有东西）、几个名字差一点的目录、一个名字对得上的文件。
    fn laid_out(&self) {
        for name in ["miyu-call-1", "miyu-call-2", "miyu-call-3"] {
            fs::create_dir_all(self.0.join(name).join("bin")).expect("建得了");
        }
        fs::write(self.0.join("miyu-call-1/bin/miyu.exe"), "").expect("写得进");
        for name in [
            "miyu-call-",
            "miyu-call-x",
            "miyu-call-+4",
            "miyu-call--5",
            "other-6",
        ] {
            fs::create_dir_all(self.0.join(name)).expect("建得了");
        }
        fs::write(self.0.join("miyu-call-7"), "").expect("写得进");
    }

    /// 还在的几样，排好序。
    fn left(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .expect("读得出")
            .map(|entry| {
                entry
                    .expect("读得出")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 只删名字对得上、是目录、进程不在了的：里面有东西的整个删；名字差一点的、文件、进程还在的不动。
#[test]
fn only_directories_of_gone_processes_go() {
    let scratch = Scratch::new();
    scratch.laid_out();
    let asked = std::sync::Mutex::new(Vec::new());
    sweep(
        &scratch.0,
        |pid| {
            asked.lock().expect("没毒").push(pid);
            pid == 2
        },
        |_| true,
    );
    assert_eq!(
        scratch.left(),
        [
            "miyu-call-",
            "miyu-call-+4",
            "miyu-call--5",
            "miyu-call-2",
            "miyu-call-7",
            "miyu-call-x",
            "other-6"
        ]
    );
    let mut asked = asked.into_inner().expect("没毒");
    asked.sort_unstable();
    assert_eq!(asked, [1, 2, 3], "只问名字对得上的目录");
}

/// 不是自己的不删，进程不在了也不删。
#[test]
fn directories_that_are_not_ours_stay() {
    let scratch = Scratch::new();
    scratch.laid_out();
    let before = scratch.left();
    sweep(&scratch.0, |_| false, |_| false);
    assert_eq!(scratch.left(), before);
}

/// 进程号的写法：前缀后面全是数字、放得进 `u32`。
#[test]
fn the_pid_is_plain_digits() {
    assert_eq!(pid_of("miyu-call-1234"), Some(1234));
    assert_eq!(pid_of("miyu-call-0"), Some(0));
    for name in [
        "miyu-call-",
        "miyu-call-+1",
        "miyu-call--1",
        "miyu-call-1a",
        "miyu-call-99999999999",
        "miyu-calls-1",
        "x-miyu-call-1",
    ] {
        assert_eq!(pid_of(name), None, "{name}");
    }
}

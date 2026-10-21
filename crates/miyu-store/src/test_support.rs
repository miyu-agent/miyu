//! 测试用的：临时目录。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 测试用的临时目录：进程号加序号，用完删掉。
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-store-test-{}-{n}", std::process::id())))
    }

    /// 这个临时目录的路径（还没建）。
    pub(crate) fn path(&self) -> &Path {
        &self.0
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

/// 在 `at` 造一个指到 `target` 的符号链接（施工 X-37）：指到的是目录的照目录造（Windows 上分两种），还没有的照文件造。
/// Windows 上没开开发者模式造不了：交回假，测试跳过（CI 上应该造得了，造不了算错）。
pub(crate) fn link(target: &Path, at: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, at);
    #[cfg(windows)]
    let made = if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, at)
    } else {
        std::os::windows::fs::symlink_file(target, at)
    };
    match made {
        Ok(()) => true,
        Err(error) => {
            assert!(
                std::env::var_os("CI").is_none(),
                "CI 上应该造得了符号链接：{error}"
            );
            eprintln!("造不了符号链接（{error}），跳过：Windows 上要管理员权限或开发者模式");
            false
        }
    }
}

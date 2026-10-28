//! 测试用的：临时目录。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 测试用的临时目录：进程号加序号，用完删掉。
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    /// 建好、写上数据根的标记（施工 TMP-1）：数据根里的东西只在数据根里建目录，测试在里面写日志、blob、库都照真的来。
    pub(crate) fn new() -> Scratch {
        let scratch = Scratch::bare();
        fs::create_dir_all(&scratch.0).expect("建得了临时目录");
        fs::write(scratch.0.join(crate::root::MARKER), "").expect("写得进标记");
        scratch
    }

    /// 还没建、没有标记的（测试数据根本身怎么建、没有数据根的地方建不建）。
    pub(crate) fn bare() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-store-test-{}-{n}", std::process::id())))
    }

    /// 这个临时目录的路径。
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// 同 [`Scratch::path`]（施工 TMP-1 时开库的测试用过它，那时临时目录还没标记）。
    pub(crate) fn rooted(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        crate::testkit::remove_tree(&self.0);
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

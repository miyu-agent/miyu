//! 测试用的临时目录：用完删掉。路径照真实位置给（macOS 的 `/var` 是链接）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 一个临时目录。
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-source-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了临时目录");
        Scratch(std::fs::canonicalize(&dir).expect("临时目录在"))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "测试收尾删不掉就留着，不碍事"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 在 `root` 下写一个文件，上一层没有的建上。
pub(crate) fn put(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
    std::fs::write(path, bytes).expect("写得进");
}

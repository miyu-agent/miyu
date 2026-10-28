//! 端点自己的单元测试共用的：临时的数据根、出厂的资源目录。

use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 临时的数据根的目录：丢掉时删掉（施工 TMP-1）。拿着它的时候数据根在。
pub(crate) struct TempDir(PathBuf);

impl Deref for TempDir {
    type Target = PathBuf;

    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl AsRef<std::path::Path> for TempDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // 删到一半撞上后台写入的过一会儿再删（施工 TMP-1）。
        miyu_store::testkit::remove_tree(&self.0);
    }
}

/// 一个临时的数据根，建好了骨架；目录名带 `name`、进程号和序号，几个测试不撞。交回数据根和它的目录，目录丢掉时删掉。
pub(crate) fn temp_root(name: &str) -> (DataRoot, TempDir) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("miyu-endpoint-{name}-{}-{n}", std::process::id()));
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(dir.clone().into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("建得了骨架");
    (root, TempDir(dir))
}

/// 源码树里出厂的资源目录。
pub(crate) fn resources() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

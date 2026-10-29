//! 放锁之前等完后台写盘（施工 TMP-1 补2）：读目录时另起线程写的整理好的一份，[`finish`] 交回时已经在缓存目录里了。写得慢的
//! （测试里让它先睡一会儿，像门禁满载时那样）也等。

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use super::*;
use crate::models::catalog::{FILE, Places, load};
use crate::models::digest;

/// 一个新的空目录。
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "miyu-core-pending-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    std::fs::create_dir_all(dir.join("snapshot")).expect("建得了");
    dir
}

#[test]
fn the_catalog_digest_is_written_before_finish_returns() {
    digest::SLOW.store(true, Ordering::Relaxed);
    let dir = scratch();
    let catalog = r#"{"p": {"id": "p", "models": {"m": {"limit": {"context": 1000}}}}}"#;
    std::fs::write(dir.join("snapshot").join(FILE), catalog).expect("写得进");
    let places = Places {
        snapshot: dir.join("snapshot"),
        cache: Some(dir.join("cache")),
        program: std::env::current_exe().ok(),
    };
    assert!(load(&places).is_some(), "读得出来");
    finish();
    assert!(
        dir.join("cache").join(digest::FILE).is_file(),
        "整理好的一份在放锁之前写完"
    );
    std::fs::remove_dir_all(dir).expect("删得掉");
}

#[test]
fn finish_waits_for_every_write_held() {
    let dir = scratch();
    let file = dir.join("written");
    let target = file.clone();
    hold(std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        std::fs::write(target, b"x").expect("写得进");
    }));
    finish();
    assert!(file.is_file());
    std::fs::remove_dir_all(dir).expect("删得掉");
}

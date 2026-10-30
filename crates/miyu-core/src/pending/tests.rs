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

/// 两处同时等（施工 TMP-3 补）：另一处先把登记取走、正在等它写完，这一处也要等到写完才交回。原来登记是整个进程一份、谁取走
/// 谁等：后到的拿到空的马上交回，写盘的线程还在睡（960c21421 的 Windows 门禁，`finish_waits_for_every_write_held` 撞上了
/// TMP-3 那条测试的 `finish`）。
#[test]
fn finish_waits_even_when_another_finish_took_the_writes() {
    let dir = scratch();
    let file = dir.join("written");
    let target = file.clone();
    hold(std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::write(target, b"x").expect("写得进");
    }));
    let other = std::thread::spawn(finish);
    // 让另一处先取走登记。
    std::thread::sleep(std::time::Duration::from_millis(100));
    finish();
    let written = file.is_file();
    other.join().expect("另一处等完了");
    std::fs::remove_dir_all(dir).expect("删得掉");
    assert!(written, "另一处取走了登记，这一处也要等到写完");
}

/// 核心起来就另起线程读目录（`read_early`），读完才起写盘的线程、登记。核心在它读完以前就停下的：放锁时登记里还是空的，
/// 夹具删了数据根，它读完又把缓存目录建回来（施工 TMP-3，main b47ecacb3 的门禁）。
#[test]
fn finish_waits_for_a_catalog_read_still_going() {
    crate::models::catalog::SLOW.store(true, Ordering::Relaxed);
    // 快照在资源目录里，留着；缓存目录在数据根里，夹具删的是它。
    let (resources, home) = (scratch(), scratch());
    let catalog = r#"{"p": {"id": "p", "models": {"m": {"limit": {"context": 1000}}}}}"#;
    std::fs::write(resources.join("snapshot").join(FILE), catalog).expect("写得进");
    let early = crate::models::read_early(
        resources.join("snapshot"),
        Some(home.join("cache")),
        std::env::current_exe().ok(),
    );
    finish();
    std::fs::remove_dir_all(&home).expect("删得掉");
    drop(early);
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let left = home.join("cache").join(digest::FILE);
    let found = left.is_file();
    drop(std::fs::remove_dir_all(&home));
    std::fs::remove_dir_all(&resources).expect("删得掉");
    assert!(
        !found,
        "放锁以后没人再写：删掉的数据根里又出现了 {}",
        left.display()
    );
}

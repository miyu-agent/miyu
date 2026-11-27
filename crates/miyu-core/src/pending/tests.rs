//! 放锁之前等完后台写盘（施工 TMP-1 补2）：读目录时另起线程写的整理好的一份，[`finish`] 交回时已经在缓存目录里了。写得慢的
//! （测试里让它先睡一会儿，像门禁满载时那样）也等。关门（[`close`]，施工 CS-8）的几条用自己的一份 [`Pending`]：整个进程那一份
//! 关了门，同一个进程里别的测试就写不了了。

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
    write(|writing| {
        Some(std::thread::spawn(move || {
            let _writing = writing;
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::fs::write(target, b"x").expect("写得进");
        }))
    });
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
    write(|writing| {
        Some(std::thread::spawn(move || {
            let _writing = writing;
            std::thread::sleep(std::time::Duration::from_millis(300));
            std::fs::write(target, b"x").expect("写得进");
        }))
    });
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
        None,
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

/// 自己的一份（施工 CS-8）：关门不碍着整个进程那一份。
fn own() -> &'static Pending {
    Box::leak(Box::new(Pending::new()))
}

/// 起一个写盘线程：等 `go` 放行再写 `file`，交回起没起（门关了的不起）。
fn writer(pending: &'static Pending, file: PathBuf, go: std::sync::mpsc::Receiver<()>) -> bool {
    let mut started = false;
    pending.write(|writing| {
        started = true;
        Some(std::thread::spawn(move || {
            let _writing = writing;
            let _go = go.recv();
            std::fs::write(file, b"x").expect("写得进");
        }))
    });
    started
}

/// 关门不等还在读的（施工 CS-8）：读目录压力下要一两分钟，收到停的信号的核心原来一直等它（2026-11-27 基准测试 B1 第 5 次）。
/// 关了门以后读完的不再起写盘的线程，删掉的数据根不会被建回来。
#[test]
fn close_leaves_a_read_still_going_and_shuts_its_write_out() {
    let pending = own();
    let reading = pending.working();
    let (done, closed) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        pending.close();
        let _sent = done.send(());
    });
    let waited = closed.recv_timeout(std::time::Duration::from_secs(10));
    assert!(waited.is_ok(), "关门不等还在读的");
    let dir = scratch();
    let (_go, gate) = std::sync::mpsc::channel();
    assert!(
        !writer(pending, dir.join("written"), gate),
        "关了门，读完的不再起写盘的线程"
    );
    drop(reading);
    std::fs::remove_dir_all(dir).expect("删得掉");
}

/// 关门照旧等已经起了的写盘线程写完（施工 TMP-1 补2）：放锁以后没人再写数据根。
#[test]
fn close_waits_for_a_write_already_started() {
    let pending = own();
    let dir = scratch();
    let file = dir.join("written");
    let (go, gate) = std::sync::mpsc::channel();
    assert!(writer(pending, file.clone(), gate), "门开着的起得来");
    let (done, closed) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        pending.close();
        let _sent = done.send(());
    });
    let early = closed.recv_timeout(std::time::Duration::from_millis(200));
    go.send(()).expect("写盘的线程等着");
    let waited = closed.recv_timeout(std::time::Duration::from_secs(10));
    let written = file.is_file();
    std::fs::remove_dir_all(dir).expect("删得掉");
    assert!(early.is_err(), "写完以前不交回");
    assert!(waited.is_ok() && written, "写完了才交回");
}

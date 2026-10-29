use super::*;
use crate::test_support::Scratch;

#[test]
fn a_second_lock_waits_for_the_first_to_go() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let first = Lock::acquire(&root).expect("第一把拿得到");
    assert!(root.run().join(FILE).is_file());
    let error = Lock::acquire(&root).expect_err("第二把拿不到");
    assert!(matches!(error, OpenError::Running), "{error:?}");
    drop(first);
    Lock::acquire(&root).expect("第一把放开以后拿得到");
}

#[test]
fn two_data_roots_do_not_share_a_lock() {
    let (one, two) = (Scratch::new(), Scratch::new());
    let _first = Lock::acquire(&one.root()).expect("拿得到");
    Lock::acquire(&two.root()).expect("另一个数据根照样拿得到");
}

/// 数据根里的锁文件还是拿着的那一个（施工 TMP-2）：删了的不是；Unix 上删了又建了一个新的也不是。
#[test]
fn the_lock_knows_when_its_file_is_gone() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let lock = Lock::acquire(&root).expect("拿得到");
    let held = lock.held();
    assert!(held.here(), "刚拿到");
    #[cfg(unix)]
    {
        std::fs::remove_file(root.run().join(FILE)).expect("删得掉");
        assert!(!held.here(), "删了");
        std::fs::write(root.run().join(FILE), "").expect("建得了");
        assert!(!held.here(), "又建了一个新的");
    }
    drop(lock);
}

/// 等核心放锁（施工 CL-4）：拿着的时候等着，放了就醒；没有锁文件的当已经放了；等完新核心照旧拿得到。
#[test]
fn waiting_returns_once_the_lock_is_let_go() {
    let scratch = Scratch::new();
    let root = scratch.root();
    wait_released(&root).expect("没有锁文件的当已经放了");
    let lock = Lock::acquire(&root).expect("拿得到");
    let (done, finished) = std::sync::mpsc::channel();
    let waiting = {
        let root = root.clone();
        std::thread::spawn(move || {
            let result = wait_released(&root);
            done.send(()).expect("等的一头在");
            result
        })
    };
    assert!(
        finished
            .recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "拿着的时候不醒"
    );
    drop(lock);
    finished
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("放了就醒");
    waiting.join().expect("没崩").expect("等成了");
    Lock::acquire(&root).expect("等完照旧拿得到");
}

//! 换桥以后认新桥（施工 test-pid 补，`onebot.md` 第 235 条）：认新桥不比进程号；换桥以前清掉旧桥的状态文件，文件在就是新桥写的。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_onebot::status_file;
use miyu_store::root::DataRoot;

use crate::support::spawning::{forget_status, is_up};
use crate::support::temp_root;

/// 写一份状态文件：进程号 `pid`，NapCat 连没连着照 `connected`。
fn write(root: &DataRoot, pid: u64, connected: bool) {
    let path = status_file::path(root);
    std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
    let napcat = json!({"connected": connected});
    let file = json!({"pid": pid, "listen": 8301, "napcat": napcat});
    std::fs::write(path, format!("{file}\n")).expect("写得了");
}

fn running(pid: u64) -> Value {
    json!({"state": "running", "pid": pid})
}

#[test]
fn a_bridge_with_the_pid_of_the_stopped_one_is_recognized_once_its_file_is_there() {
    // 进程号被回收，新桥和停掉的同号：状态文件是新写的，认。
    let file = json!({"pid": 1268, "listen": 8301, "napcat": {"connected": false}});
    assert!(is_up(&running(1268), Some(&file)));
}

#[test]
fn nothing_is_recognized_before_the_file_is_written_or_with_another_pid() {
    let file = json!({"pid": 1268});
    assert!(!is_up(&running(1268), None), "清掉了、还没写");
    assert!(!is_up(&running(1269), Some(&file)), "文件是别的进程的");
    assert!(
        !is_up(&running(1268), Some(&json!({"listen": 8301}))),
        "文件没有进程号"
    );
    assert!(
        !is_up(&running(1268), Some(&json!({"pid": "1268"}))),
        "进程号不是数"
    );
}

#[test]
fn only_a_running_one_with_a_pid_is_recognized() {
    let file = json!({"pid": 1268});
    for state in ["starting", "stopped", "backoff", "off"] {
        assert!(
            !is_up(&json!({"state": state, "pid": 1268}), Some(&file)),
            "{state}"
        );
    }
    assert!(
        !is_up(&json!({"state": "running"}), Some(&json!({}))),
        "没有进程号"
    );
}

#[tokio::test]
async fn forgetting_removes_the_file_of_a_bridge_napcat_has_left() {
    let (dir, root) = temp_root();
    write(&root, 7, false);
    forget_status(&root).await;
    assert_eq!(status_file::read(&root), None);
    assert!(!status_file::path(&root).exists());
    std::fs::remove_dir_all(dir).expect("删得了");
}
#[tokio::test]
async fn forgetting_without_a_file_is_fine() {
    let (dir, root) = temp_root();
    forget_status(&root).await;
    assert!(!status_file::path(&root).exists());
    std::fs::remove_dir_all(dir).expect("删得了");
}
#[tokio::test]
async fn forgetting_waits_for_the_old_bridge_to_write_that_napcat_left() {
    let (dir, root) = temp_root();
    write(&root, 7, true);
    let later = root.clone();
    let rewriting = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        write(&later, 7, false);
    });
    let started = std::time::Instant::now();
    forget_status(&root).await;
    rewriting.await.expect("写完了");
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "NapCat 还连着时不删：旧桥断开后的那次写会把它写回来"
    );
    assert!(!status_file::path(&root).exists());
    std::fs::remove_dir_all(dir).expect("删得了");
}

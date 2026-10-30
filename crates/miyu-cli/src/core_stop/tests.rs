use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use miyu_ipc::{Dirs, Lock};
use miyu_store::env::{Env, Platform};

use super::*;

/// 一个用完就删的临时数据根，建好了骨架。
fn temp_root(name: &str) -> DataRoot {
    let dir =
        std::env::temp_dir().join(format!("miyu-cli-core-stop-{name}-{}", std::process::id()));
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(dir.into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("建得了骨架");
    root
}

/// 回了「停」以后要等锁放开才回（施工 CL-4）：假核心答完 `core.stop` 还拿着锁一会儿，`miyu core stop` 回来时它已经放了；
/// 什么都不印，退出码 0。
#[tokio::test]
async fn stop_returns_only_after_the_lock_is_let_go() {
    stops_cleanly("wait", true).await;
}

/// 核心收到 `core.stop` 停得快，回应还没送到连接就断了（CL-4 修：CI 上偶发「与核心的连接已断开」、退出码 1）：照样当它接了，
/// 等锁放开，什么都不印，退出码 0。
#[tokio::test]
async fn a_core_that_hangs_up_before_answering_stop_still_counts() {
    stops_cleanly("hangup", false).await;
}

/// 起一个假核心：答 `hello`；`core.stop` 照 `answer` 答或者不答就断开，再拿着锁 300 毫秒才放。`miyu core stop` 照样成、锁放了才回。
async fn stops_cleanly(name: &str, answer: bool) {
    let root = temp_root(name);
    let opened = miyu_ipc::open_locked(
        &root,
        &Dirs::current(),
        Lock::acquire(&root).expect("拿得到锁"),
    )
    .expect("开得了套接字");
    let released = Arc::new(AtomicBool::new(false));
    let core = {
        let released = Arc::clone(&released);
        tokio::spawn(async move {
            let mut listener = opened.listener;
            let connection = listener.accept().await.expect("连得上");
            let (read, mut write) = tokio::io::split(connection);
            let mut lines = BufReader::new(read).lines();
            let mut asked = Vec::new();
            while let Some(line) = lines.next_line().await.expect("读得了") {
                let request: Value = serde_json::from_str(&line).expect("是 JSON");
                let method = request["method"].as_str().unwrap_or_default().to_string();
                asked.push(method.clone());
                if method == "core.stop" && !answer {
                    break;
                }
                let reply = json!({"jsonrpc": "2.0", "id": request["id"], "result": {}});
                write
                    .write_all(format!("{reply}\n").as_bytes())
                    .await
                    .expect("写得出");
                if method == "core.stop" {
                    break;
                }
            }
            drop((lines, write));
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            released.store(true, Ordering::SeqCst);
            drop(listener);
            asked
        })
    };
    let mut err = Vec::new();
    let code = core_stop_on(&root, Language::Chinese, &mut err).await;
    assert!(released.load(Ordering::SeqCst), "锁放了才回");
    assert_eq!(code, exit::OK, "{}", String::from_utf8_lossy(&err));
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    assert_eq!(core.await.expect("没崩"), ["hello", "core.stop"]);
    std::fs::remove_dir_all(root.path()).expect("删得掉临时目录");
}

/// 没在跑的：什么都不说，退出码 0。
#[tokio::test]
async fn nothing_running_is_nothing_to_stop() {
    let root = temp_root("idle");
    let mut err = Vec::new();
    assert_eq!(
        core_stop_on(&root, Language::Chinese, &mut err).await,
        exit::OK
    );
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    std::fs::remove_dir_all(root.path()).expect("删得掉临时目录");
}

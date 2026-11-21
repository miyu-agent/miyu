//! 整个进程的监视共用一组线程（施工 LW-11）：常驻的核心监视的六份同时在，只有一条 `notify` 的线程、一条 `miyu-watch`。
//! 数的是整个进程的线程，自己一个程序：和别的测试并着跑，别人开的监视也算进来。只在 Linux 上数（`/proc/self/task`）。

#![cfg(target_os = "linux")]

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use miyu_store::watch::{Watch, watch, watch_dir, watch_tree};
use support::Scratch;

/// 等一次交出来的最多等多久。
const WAIT: Duration = Duration::from_secs(10);

/// 这个进程里名字照 `named` 认的线程有几条（Linux 的线程名最多 15 个字节，`notify-rs inotify loop` 存成 `notify-rs inoti`）。
fn threads(named: impl Fn(&str) -> bool) -> usize {
    fs::read_dir("/proc/self/task")
        .expect("读得了 /proc/self/task")
        .filter_map(|task| fs::read_to_string(task.ok()?.path().join("comm")).ok())
        .filter(|name| named(name.trim_end()))
        .count()
}

/// 去抖的那一条。
fn watchers() -> usize {
    threads(|name| name == "miyu-watch")
}

/// `notify` 自己的。
fn notifiers() -> usize {
    threads(|name| name.starts_with("notify-rs"))
}

/// 交出来的都送进一条管道。
fn collect() -> (impl Fn(&Path) + Send + 'static, Receiver<PathBuf>) {
    let (send, received) = channel();
    let send = Mutex::new(send);
    let on_change = move |path: &Path| {
        send.lock()
            .expect("锁没坏")
            .send(path.to_path_buf())
            .expect("收的那一头在");
    };
    (on_change, received)
}

/// 等到交出 `wanted` 为止。
fn wait_for(received: &Receiver<PathBuf>, wanted: &Path) -> bool {
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match received.recv_timeout(left) {
            Ok(path) if path == wanted => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

/// 照常驻的核心订那六份（`miyu-endpoint` 的 `watch_config`、`watch_scripts`、`watch_edits`）：配置、信任、密钥一份，脚本
/// 目录一份，你那一层的四棵树各一份。交回监视、每一份改哪个文件、等交出哪个路径、收的那一头。
fn six(root: &Path) -> Vec<(Watch, PathBuf, PathBuf, Receiver<PathBuf>)> {
    let etc = root.join("etc");
    let private = etc.join("private");
    let user = root.join("root");
    for dir in [&etc, &private, &user.join("scripts")] {
        fs::create_dir_all(dir).expect("建得了目录");
    }
    let mut six = Vec::new();
    let config = etc.join("config.toml");
    let files = [
        config.clone(),
        etc.join("trust.toml"),
        private.join("secrets.toml"),
    ];
    let (on_change, received) = collect();
    six.push((
        watch(&files, on_change).expect("监视得了"),
        config.clone(),
        config,
        received,
    ));
    let scripts = user.join("scripts");
    let (on_change, received) = collect();
    let script = scripts.join("weather.py");
    six.push((
        watch_dir(&scripts, on_change).expect("监视得了"),
        script,
        scripts,
        received,
    ));
    for tree in ["tools", "prompts", "personas", "presets"] {
        let dir = user.join(tree);
        fs::create_dir_all(&dir).expect("建得了目录");
        let file = dir.join("mine.txt");
        let (on_change, received) = collect();
        six.push((
            watch_tree(&dir, on_change).expect("监视得了"),
            file.clone(),
            file,
            received,
        ));
    }
    six
}

#[test]
fn six_subscriptions_share_one_pair_of_threads() {
    assert_eq!((watchers(), notifiers()), (0, 0), "还没有人订时一条都不开");
    let temp = Scratch::new("watch-threads");
    let six = six(&temp.0);
    for (watching, ..) in &six {
        assert_eq!(watching.unavailable(), None);
    }
    assert_eq!(
        (watchers(), notifiers()),
        (1, 1),
        "六份共用一条 miyu-watch、一条 notify 的线程"
    );
    for (_, file, wanted, received) in &six {
        fs::write(file, "x\n").unwrap();
        assert!(wait_for(received, wanted), "{wanted:?}");
    }
    drop(six);
    // 监视中心常驻：都退订了也不再开、不再关。
    assert_eq!((watchers(), notifiers()), (1, 1));
    let again = Scratch::new("watch-threads-again");
    let six = self::six(&again.0);
    assert_eq!((watchers(), notifiers()), (1, 1), "再订一遍还是这一组");
    for (_, file, wanted, received) in &six {
        fs::write(file, "y\n").unwrap();
        assert!(wait_for(received, wanted), "{wanted:?}");
    }
}

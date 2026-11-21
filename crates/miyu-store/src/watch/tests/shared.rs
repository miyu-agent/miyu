//! 整个进程共用一组监视线程（施工 LW-11，`hub.rs`）：几份订阅同时存在时，每一份的行为和原来各开各的线程时一样。

use super::*;

/// 等到 `received` 交出 `wanted` 为止（中间交的别的不管），等得到的交 `true`。
fn wait_for(received: &Receiver<PathBuf>, wanted: &Path) -> bool {
    let deadline = Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match received.recv_timeout(left) {
            Ok(path) if path == wanted => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

/// 先把已经交出来的收干净，再等五个去抖的时长：这中间什么都没交出来的交 `true`。
fn quiet_after(received: &Receiver<PathBuf>) -> bool {
    received.recv_timeout(QUIET * 5).is_err()
}

/// 一份订阅交得慢（卡在 `on_change` 里），别的订阅照样收到；慢的那一份自己的一个接一个地交，不会同时交两个。
#[test]
fn a_stuck_subscription_does_not_hold_up_the_others() {
    let (_temp, slow_dir) = scratch();
    let (_other, fast_dir) = scratch();
    let (entered, entering) = channel::<()>();
    let (release, released) = channel::<()>();
    let (entered, released) = (Mutex::new(entered), Mutex::new(released));
    let inside = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let most = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (inside_seen, most_seen) = (Arc::clone(&inside), Arc::clone(&most));
    let _slow = watch_dir(&slow_dir, move |_| {
        use std::sync::atomic::Ordering::SeqCst;
        let now = inside_seen.fetch_add(1, SeqCst) + 1;
        most_seen.fetch_max(now, SeqCst);
        entered.lock().unwrap().send(()).unwrap();
        // 测试收尾时放的那一头没了：不再等。
        if released.lock().unwrap().recv().is_err() {
            return;
        }
        inside_seen.fetch_sub(1, SeqCst);
    })
    .unwrap();
    let (on_change, received) = collect();
    let _fast = watch_dir(&fast_dir, on_change).unwrap();
    fs::write(slow_dir.join("a.py"), "1\n").unwrap();
    entering
        .recv_timeout(WAIT)
        .expect("慢的那一份进了 on_change");
    // 它卡着的时候：别的订阅照样交；它自己又变了的排着。
    fs::write(fast_dir.join("b.py"), "1\n").unwrap();
    assert!(wait_for(&received, &fast_dir), "别的订阅不受它连累");
    fs::write(slow_dir.join("a.py"), "2\n").unwrap();
    std::thread::sleep(QUIET * 3);
    release.send(()).unwrap();
    entering
        .recv_timeout(WAIT)
        .expect("放开以后排着的那一次照样交");
    release.send(()).unwrap();
    assert_eq!(
        most.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "同一份订阅不会同时交两个"
    );
}

/// 丢掉一份订阅：它不再收到，别的不受影响。
#[test]
fn a_dropped_subscription_stops_and_the_others_go_on() {
    let (_temp, gone_dir) = scratch();
    let (_other, kept_dir) = scratch();
    let (gone_change, gone) = collect();
    let (kept_change, kept) = collect();
    let dropped = watch_dir(&gone_dir, gone_change).unwrap();
    let _kept = watch_dir(&kept_dir, kept_change).unwrap();
    drop(dropped);
    fs::write(gone_dir.join("a.py"), "1\n").unwrap();
    fs::write(kept_dir.join("b.py"), "1\n").unwrap();
    assert!(wait_for(&kept, &kept_dir));
    assert!(quiet_after(&gone), "丢掉了的不再交");
}

/// 同一个目录被两份订阅看：各认各的文件；先订的、后订的哪一份先走，另一份都照样收到。
#[test]
fn two_subscriptions_on_one_directory_each_see_their_own_and_either_can_leave() {
    for first_leaves in [true, false] {
        let (_temp, dir) = scratch();
        let config = dir.join("config.toml");
        let trust = dir.join("trust.toml");
        let (config_change, configs) = collect();
        let (trust_change, trusts) = collect();
        let config_watch = watch(std::slice::from_ref(&config), config_change).unwrap();
        let trust_watch = watch(std::slice::from_ref(&trust), trust_change).unwrap();
        assert_eq!(config_watch.unavailable(), None);
        assert_eq!(trust_watch.unavailable(), None);
        save(&config, "[ui]\n");
        assert!(wait_for(&configs, &config));
        save(&trust, "[ui]\n");
        assert!(wait_for(&trusts, &trust));
        assert!(
            configs.try_iter().all(|path| path == config),
            "别的名字不交给它"
        );
        if first_leaves {
            drop(config_watch);
            save(&trust, "[log]\n");
            assert!(wait_for(&trusts, &trust), "先订的走了，后订的照样收到");
        } else {
            drop(trust_watch);
            save(&config, "[log]\n");
            assert!(wait_for(&configs, &config), "后订的走了，先订的照样收到");
        }
    }
}

/// 一份看目录、另一份看它里面的文件：两份都收到；看目录的走了，看文件的照样收到。递归看一棵树、另一份看树里的文件也一样，
/// 先订哪一份都一样。
#[test]
fn a_directory_and_a_file_in_it_are_both_seen_and_either_can_leave() {
    let (_temp, dir) = scratch();
    let file = dir.join("config.toml");
    let (dir_change, dirs) = collect();
    let (file_change, files) = collect();
    let whole = watch_dir(&dir, dir_change).unwrap();
    let _one = watch(std::slice::from_ref(&file), file_change).unwrap();
    save(&file, "[ui]\n");
    assert!(wait_for(&dirs, &dir));
    assert!(wait_for(&files, &file));
    drop(whole);
    save(&file, "[log]\n");
    assert!(wait_for(&files, &file), "看目录的走了，看文件的照样收到");

    for tree_first in [true, false] {
        let (_temp, dir) = scratch();
        let deep = dir.join("core");
        fs::create_dir_all(&deep).unwrap();
        let file = deep.join("config.toml");
        let (tree_change, trees) = collect();
        let (file_change, files) = collect();
        let (tree, one) = match tree_first {
            true => {
                let tree = watch_tree(&dir, tree_change).unwrap();
                (
                    tree,
                    watch(std::slice::from_ref(&file), file_change).unwrap(),
                )
            }
            false => {
                let one = watch(std::slice::from_ref(&file), file_change).unwrap();
                (watch_tree(&dir, tree_change).unwrap(), one)
            }
        };
        fs::write(&file, "[ui]\n").unwrap();
        assert!(wait_for(&trees, &file), "树交的是那个文件");
        assert!(wait_for(&files, &file));
        drop(tree);
        fs::write(&file, "[log]\n").unwrap();
        assert!(wait_for(&files, &file), "树走了，看文件的照样收到");
        drop(one);
    }
}

/// 目录还没有时订的（Linux 上 inotify 拒绝，退回轮询）：别的订阅照样用系统的监视、照样收到；目录后来建起来，轮询的看得到。
#[cfg(target_os = "linux")]
#[test]
fn a_subscription_falling_back_to_polling_leaves_the_others_alone() {
    let (_temp, there) = scratch();
    let (there_change, theres) = collect();
    let present = watch_dir(&there, there_change).unwrap();
    assert_eq!(present.unavailable(), None);
    let temp = Scratch::new();
    let later = temp.path().join("later");
    let file = later.join("config.toml");
    let (later_change, laters) = collect();
    let polling = watch(std::slice::from_ref(&file), later_change).unwrap();
    assert!(polling.unavailable().is_some(), "说得出为什么起不来");
    fs::write(there.join("a.py"), "1\n").unwrap();
    assert!(wait_for(&theres, &there), "别的订阅照样收到");
    fs::create_dir_all(&later).unwrap();
    fs::write(&file, "[ui]\n").unwrap();
    assert!(wait_for(&laters, &file), "后来建起来的照样看得到");
}

/// 目录被删了又建起来，之后再订的看的是新的那一个；之前订的那一份走了也不碍事。Windows 上看着的目录删不干净（开着的句柄让它
/// 停在待删），不在那里跑。
#[cfg(unix)]
#[test]
fn a_directory_deleted_and_made_again_is_watched_by_a_new_subscription() {
    let (_temp, dir) = scratch();
    let (old_change, olds) = collect();
    let old = watch_dir(&dir, old_change).unwrap();
    fs::write(dir.join("a.py"), "1\n").unwrap();
    assert!(wait_for(&olds, &dir));
    fs::remove_dir_all(&dir).unwrap();
    fs::create_dir_all(&dir).unwrap();
    let (new_change, news) = collect();
    let _new = watch_dir(&dir, new_change).unwrap();
    fs::write(dir.join("b.py"), "1\n").unwrap();
    assert!(wait_for(&news, &dir), "新订的看的是新的目录");
    drop(old);
    fs::write(dir.join("c.py"), "1\n").unwrap();
    assert!(wait_for(&news, &dir), "旧的那一份走了，新订的照样收到");
}

/// 一份订阅交的时候 panic 了：它停下、不再交，别的订阅照样收到。
#[test]
fn a_panicking_subscription_stops_alone() {
    let (_temp, bad_dir) = scratch();
    let (_other, good_dir) = scratch();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let _bad = watch_dir(&bad_dir, move |_| {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        panic!("这一份交的时候崩了（测试故意的）");
    })
    .unwrap();
    let (on_change, received) = collect();
    let _good = watch_dir(&good_dir, on_change).unwrap();
    fs::write(bad_dir.join("a.py"), "1\n").unwrap();
    let deadline = Instant::now() + WAIT;
    while calls.load(std::sync::atomic::Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    fs::write(good_dir.join("b.py"), "1\n").unwrap();
    assert!(wait_for(&received, &good_dir), "别的订阅不受它连累");
    fs::write(bad_dir.join("a.py"), "2\n").unwrap();
    fs::write(good_dir.join("b.py"), "2\n").unwrap();
    assert!(wait_for(&received, &good_dir));
    std::thread::sleep(QUIET * 3);
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "崩过的那一份不再交"
    );
}

/// 往坏里测：同一个文件连着改三百次，合成很少的几次，最后一次改完以后一定交过。
#[test]
fn hundreds_of_changes_to_one_file_are_merged() {
    let (_temp, dir) = scratch();
    let file = dir.join("config.toml");
    let seen: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let kept = Arc::clone(&seen);
    let _watching = watch(std::slice::from_ref(&file), move |_| {
        kept.lock().unwrap().push(Instant::now());
    })
    .unwrap();
    for n in 0..300 {
        fs::write(&file, format!("# {n}\n")).unwrap();
    }
    let last = Instant::now();
    let deadline = last + WAIT;
    while !seen.lock().unwrap().iter().any(|at| *at >= last) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|at| *at >= last), "最后一次改完以后交过");
    // 只有隔了 200 毫秒以上的才分开交：一口气写完的三百次，再忙的机器也隔不出十次。
    assert!(seen.len() <= 10, "合成了：{}", seen.len());
}

/// 往坏里测：六份订阅同时在，几个线程同时往各自的目录里写几百个文件，每一份都收到，最后一次写完以后都交过。
#[test]
fn six_subscriptions_under_a_storm_each_hear_their_own() {
    let temps: Vec<(Scratch, PathBuf)> = (0..6).map(|_| scratch()).collect();
    let mut watches = Vec::new();
    let mut receivers = Vec::new();
    for (_, dir) in &temps {
        let (on_change, received) = collect();
        watches.push(watch_dir(dir, on_change).unwrap());
        receivers.push(received);
    }
    let writers: Vec<_> = temps
        .iter()
        .map(|(_, dir)| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                for n in 0..100 {
                    fs::write(dir.join(format!("{n}.py")), "1\n").unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    for ((_, dir), received) in temps.iter().zip(&receivers) {
        assert!(wait_for(received, dir), "{dir:?}");
        assert!(received.try_iter().all(|path| path == *dir), "只交自己的");
    }
    drop(watches);
}

/// 往坏里测：一边改，一边有别的订阅订上又退掉一百次；一直在的那一份照样收到，订上又退掉的每一次都看得到自己的目录。
#[test]
fn subscribing_and_leaving_while_changes_flow() {
    let (_temp, dir) = scratch();
    let (on_change, received) = collect();
    let _steady = watch_dir(&dir, on_change).unwrap();
    let churn = std::thread::spawn(|| {
        let (_temp, other) = scratch();
        for n in 0..100 {
            let (on_change, received) = collect();
            let watching = watch_dir(&other, on_change).unwrap();
            assert_eq!(watching.unavailable(), None);
            if n % 10 == 0 {
                fs::write(other.join(format!("{n}.py")), "1\n").unwrap();
                assert!(wait_for(&received, &other), "第 {n} 次订上的看得到");
            }
        }
    });
    for n in 0..200 {
        fs::write(dir.join(format!("{}.py", n % 7)), "1\n").unwrap();
    }
    churn.join().unwrap();
    fs::write(dir.join("last.py"), "1\n").unwrap();
    assert!(wait_for(&received, &dir));
}

//! 监视：先写新文件再改名的存法认得出；200 毫秒合并；别的文件名、只读的动静不理；链接指向的目录也看；照真实的位置比
//! （macOS 的临时目录在 `/var` 下，它是链接）；系统的监视起不来的轮询也认得出。
//!
//! 等变动一律有上限（10 秒），不靠固定的歇一会儿赌时序。

use std::fs;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use notify::event::{AccessKind, AccessMode, CreateKind, ModifyKind, RenameMode};

use super::*;
use crate::test_support::Scratch;

/// 等一次交出来的最多等多久。
const WAIT: Duration = Duration::from_secs(10);

/// 一份临时目录，里面建好 `dir`。
fn scratch() -> (Scratch, PathBuf) {
    let temp = Scratch::new();
    let dir = temp.path().join("system");
    fs::create_dir_all(&dir).unwrap();
    (temp, dir)
}

/// 交出来的都送进一条管道。
fn collect() -> (impl Fn(&Path) + Send + 'static, Receiver<PathBuf>) {
    let (send, received) = channel();
    let send = Mutex::new(send);
    let on_change = move |path: &Path| {
        send.lock().unwrap().send(path.to_path_buf()).unwrap();
    };
    (on_change, received)
}

/// 一个变动：种类、碰到的路径。
fn event(kind: EventKind, path: &Path) -> Event {
    Event::new(kind).add_path(path.to_path_buf())
}

/// 先写旁边的新文件、再改名盖上：编辑器、核心自己都这样存。
fn save(path: &Path, text: &str) {
    let temp = path.with_file_name(".config.toml.1-0.tmp");
    fs::write(&temp, text).unwrap();
    fs::rename(&temp, path).unwrap();
}

#[test]
fn only_the_watched_names_count_and_reads_do_not() {
    let (_temp, dir) = scratch();
    let file = dir.join("config.toml");
    let table = Table::of(std::slice::from_ref(&file));
    let modify = EventKind::Modify(ModifyKind::Any);
    assert_eq!(table.touched(&event(modify, &file)), [file.as_path()]);
    let renamed = EventKind::Modify(ModifyKind::Name(RenameMode::To));
    assert_eq!(table.touched(&event(renamed, &file)), [file.as_path()]);
    let written = EventKind::Access(AccessKind::Close(AccessMode::Write));
    assert_eq!(table.touched(&event(written, &file)), [file.as_path()]);
    for other in ["config.toml~", ".config.toml.1-0.tmp", "settings.toml"] {
        assert!(
            table.touched(&event(modify, &dir.join(other))).is_empty(),
            "{other} 不理"
        );
    }
    let elsewhere = dir.parent().unwrap().join("config.toml");
    assert!(
        table.touched(&event(modify, &elsewhere)).is_empty(),
        "别的目录里同名的不理"
    );
    for read in [
        AccessKind::Open(AccessMode::Any),
        AccessKind::Close(AccessMode::Read),
        AccessKind::Read,
    ] {
        assert!(
            table
                .touched(&event(EventKind::Access(read), &file))
                .is_empty(),
            "只读的动静不理：{read:?}"
        );
    }
}

#[test]
fn a_rescan_touches_every_file_once() {
    let (_temp, dir) = scratch();
    let files = [dir.join("config.toml"), dir.join("trust.toml")];
    let table = Table::of(&files);
    let rescan = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
    assert_eq!(
        table.touched(&rescan),
        [files[0].as_path(), files[1].as_path()]
    );
}

#[cfg(unix)]
#[test]
fn paths_are_compared_at_their_real_location() {
    let (temp, dir) = scratch();
    // 头给的是链接下的路径，系统报的是真实的位置（macOS 的 FSEvents 就这样），反过来也认得。
    let link = temp.path().join("linked");
    std::os::unix::fs::symlink(&dir, &link).unwrap();
    let file = link.join("config.toml");
    let table = Table::of(std::slice::from_ref(&file));
    let real = fs::canonicalize(&dir).unwrap().join("config.toml");
    let modify = EventKind::Modify(ModifyKind::Any);
    assert_eq!(table.touched(&event(modify, &real)), [file.as_path()]);
    assert_eq!(table.touched(&event(modify, &file)), [file.as_path()]);
}

#[cfg(unix)]
#[test]
fn a_linked_file_is_also_watched_where_it_really_is() {
    let (temp, dir) = scratch();
    let elsewhere = temp.path().join("dotfiles");
    fs::create_dir_all(&elsewhere).unwrap();
    let body = elsewhere.join("miyu.toml");
    fs::write(&body, "").unwrap();
    let file = dir.join("config.toml");
    std::os::unix::fs::symlink(&body, &file).unwrap();
    let table = Table::of(std::slice::from_ref(&file));
    let modify = EventKind::Modify(ModifyKind::Any);
    let real = fs::canonicalize(&body).unwrap();
    assert_eq!(table.touched(&event(modify, &real)), [file.as_path()]);
    assert!(
        table
            .touched(&event(modify, &real.with_file_name("other.toml")))
            .is_empty()
    );
}

#[test]
fn changes_in_a_row_are_handed_over_once_after_a_quiet_spell() {
    let (_temp, dir) = scratch();
    let file = dir.join("config.toml");
    let table = Table::of(std::slice::from_ref(&file));
    let (send, events) = channel();
    let seen: Arc<Mutex<Vec<(PathBuf, Instant)>>> = Arc::default();
    let kept = Arc::clone(&seen);
    // 合并的时长放宽到 600 毫秒：忙的机器上歇 50 毫秒可能歇得更久，别让它自己隔开。
    let quiet = QUIET * 3;
    let thread = std::thread::spawn(move || {
        run(&events, &table, quiet, &|path: &Path| {
            kept.lock()
                .unwrap()
                .push((path.to_path_buf(), Instant::now()));
        });
    });
    let kinds = [
        EventKind::Create(CreateKind::File),
        EventKind::Modify(ModifyKind::Any),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)),
    ];
    // 时刻在送之前记：`run` 从收到那一刻起算静默，送完以后这个线程可能被挂起一阵，记晚了会冤枉它。
    let mut last = Instant::now();
    for kind in kinds {
        last = Instant::now();
        send.send(Ok(event(kind, &file))).unwrap();
        std::thread::sleep(Duration::from_millis(50));
    }
    // 送事件的那一头丢掉以前等它交出来：丢掉了线程就退了。
    let deadline = Instant::now() + WAIT;
    while seen.lock().unwrap().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    // 再来一下：隔得远的另算一次。
    let again = Instant::now();
    send.send(Ok(event(kinds[1], &file))).unwrap();
    let deadline = Instant::now() + WAIT;
    while seen.lock().unwrap().len() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(send);
    thread.join().unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "连着的三下合成一次，隔开的另一次：{seen:?}");
    assert!(seen.iter().all(|(path, _)| *path == file));
    assert!(seen[0].1 >= last + quiet, "最后一下之后静够了才交");
    assert!(seen[1].1 >= again + quiet);
}

#[test]
fn a_save_by_rename_is_seen_and_other_names_are_not() {
    let (_temp, dir) = scratch();
    let file = dir.join("config.toml");
    let (on_change, received) = collect();
    let watching = watch(std::slice::from_ref(&file), on_change).unwrap();
    assert_eq!(
        watching.unavailable(),
        None,
        "这几个平台上系统的监视都起得来"
    );
    fs::write(dir.join("notes.txt"), "别的").unwrap();
    save(&file, "[ui]\nlanguage = \"zh\"\n");
    assert_eq!(received.recv_timeout(WAIT).unwrap(), file);
    drop(watching);
    let rest: Vec<PathBuf> = received.try_iter().collect();
    assert!(
        rest.iter().all(|path| *path == file),
        "只交要看的那一份：{rest:?}"
    );
}

#[test]
fn a_deleted_file_is_seen() {
    let (_temp, dir) = scratch();
    let file = dir.join("settings.toml");
    fs::write(&file, "").unwrap();
    let (on_change, received) = collect();
    let _watching = watch(std::slice::from_ref(&file), on_change).unwrap();
    fs::remove_file(&file).unwrap();
    assert_eq!(received.recv_timeout(WAIT).unwrap(), file);
}

#[cfg(unix)]
#[test]
fn a_linked_file_changed_where_it_really_is_is_seen() {
    let (temp, dir) = scratch();
    let elsewhere = temp.path().join("dotfiles");
    fs::create_dir_all(&elsewhere).unwrap();
    let body = elsewhere.join("miyu.toml");
    fs::write(&body, "").unwrap();
    let file = dir.join("config.toml");
    std::os::unix::fs::symlink(&body, &file).unwrap();
    let (on_change, received) = collect();
    let _watching = watch(std::slice::from_ref(&file), on_change).unwrap();
    fs::write(&body, "[log]\nlevel = \"debug\"\n").unwrap();
    assert_eq!(received.recv_timeout(WAIT).unwrap(), file);
}

#[cfg(unix)]
#[test]
fn a_directory_given_through_a_link_is_watched_at_its_real_location() {
    let (temp, dir) = scratch();
    let link = temp.path().join("linked");
    std::os::unix::fs::symlink(&dir, &link).unwrap();
    let file = link.join("config.toml");
    let (on_change, received) = collect();
    let _watching = watch(std::slice::from_ref(&file), on_change).unwrap();
    save(&dir.join("config.toml"), "[ui]\n");
    assert_eq!(
        received.recv_timeout(WAIT).unwrap(),
        file,
        "交的是给的那个路径"
    );
}

#[test]
fn polling_sees_changes_too() {
    let (_temp, dir) = scratch();
    let file = dir.join("config.toml");
    let (on_change, received) = collect();
    let watching = start(
        std::slice::from_ref(&file),
        Backend::Poll(Duration::from_millis(50)),
        QUIET,
        on_change,
    )
    .unwrap();
    assert_eq!(watching.unavailable(), None);
    fs::write(&file, "[ui]\nlanguage = \"en\"\n").unwrap();
    assert_eq!(received.recv_timeout(WAIT).unwrap(), file);
}

/// 系统的监视起不来（这里拿一个还没有的目录让 inotify 拒绝），退回轮询，交回原因；之后建起来的照样看得到。
#[cfg(target_os = "linux")]
#[test]
fn when_the_system_watch_fails_it_falls_back_to_polling() {
    let temp = Scratch::new();
    let dir = temp.path().join("later");
    let file = dir.join("config.toml");
    let (on_change, received) = collect();
    let watching = watch(std::slice::from_ref(&file), on_change).unwrap();
    assert!(watching.unavailable().is_some(), "说得出为什么起不来");
    fs::create_dir_all(&dir).unwrap();
    fs::write(&file, "[ui]\n").unwrap();
    assert_eq!(received.recv_timeout(WAIT).unwrap(), file);
}

/// 看一整个目录（施工 X-12 补）：点开头的以外哪个名字都算，交的是目录；点开头的、子文件夹里的、只读的不算。
#[test]
fn a_watched_directory_counts_every_plain_name_in_it() {
    let (_temp, dir) = scratch();
    let table = Table::of_dir(&dir);
    let modify = EventKind::Modify(ModifyKind::Any);
    for name in ["weather.py", "notes", "backup~"] {
        assert_eq!(
            table.touched(&event(modify, &dir.join(name))),
            [dir.as_path()],
            "{name}"
        );
    }
    for path in [dir.join(".weather.py.swp"), dir.join("deep").join("x.py")] {
        assert!(table.touched(&event(modify, &path)).is_empty(), "{path:?}");
    }
    let read = EventKind::Access(AccessKind::Open(AccessMode::Read));
    assert!(
        table
            .touched(&event(read, &dir.join("weather.py")))
            .is_empty()
    );
}

#[test]
fn a_new_and_a_deleted_file_in_a_watched_directory_are_seen() {
    let (_temp, dir) = scratch();
    let (on_change, received) = collect();
    let _watching = watch_dir(&dir, on_change).unwrap();
    fs::write(dir.join("weather.py"), "print(1)\n").unwrap();
    assert_eq!(received.recv_timeout(WAIT).unwrap(), dir);
    fs::remove_file(dir.join("weather.py")).unwrap();
    assert_eq!(received.recv_timeout(WAIT).unwrap(), dir);
}

/// 往下递归看的（施工 OV-1 下）：哪一层的文件变了都认，交出去的是那个文件；路上有点开头的那一段不算。
#[test]
fn a_watched_tree_hands_over_the_file_that_changed() {
    let (_temp, dir) = scratch();
    let table = Table::of_tree(&dir);
    let modify = EventKind::Modify(ModifyKind::Any);
    // 系统报的是真实的位置（macOS 的 `/var` 是链接，Windows 的临时目录可能是短名）：交出去的照调的一方给的拼。
    let real = dir.canonicalize().unwrap();
    let deep = Path::new("core").join("facts").join("env.txt");
    assert_eq!(
        table.touched(&event(modify, &real.join(&deep))),
        [dir.join(&deep)]
    );
    for path in [
        real.join(".git").join("x"),
        real.join("core").join(".x.swp"),
    ] {
        assert!(table.touched(&event(modify, &path)).is_empty(), "{path:?}");
    }
    assert!(
        table.touched(&event(modify, &real)).is_empty(),
        "目录自己不算"
    );
    // 还在的目录变了不算（Windows 会单独报一下目录的时间变了）；删掉了的照算。
    fs::create_dir_all(real.join("personas").join("engineer")).unwrap();
    let persona = real.join("personas").join("engineer");
    assert!(table.touched(&event(modify, &persona)).is_empty());
    fs::remove_dir_all(&persona).unwrap();
    assert_eq!(
        table.touched(&event(
            EventKind::Remove(notify::event::RemoveKind::Folder),
            &persona
        )),
        [dir.join("personas").join("engineer")]
    );
}

#[test]
fn a_file_deep_in_a_watched_tree_is_seen() {
    let (_temp, dir) = scratch();
    let deep = dir.join("core").join("facts");
    fs::create_dir_all(&deep).unwrap();
    let (on_change, received) = collect();
    let _watching = watch_tree(&dir, on_change).unwrap();
    fs::write(deep.join("env.txt"), "Mine.\n").unwrap();
    // 有的系统先报一下建过的目录（macOS 的 FSEvents）：等到那个文件为止。
    let wanted = deep.join("env.txt");
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if received.recv_timeout(left).expect("等得到那个文件") == wanted {
            break;
        }
    }
}

/// 新建、挪进来的子文件夹（施工 OV-1 补）：系统的递归监视等报了新文件夹才去看它，刚建好就写进去的文件赶在前头，报不出来；
/// 报新文件夹时把里面已经有的文件都交出去。还在的目录别的变动照旧不算。
#[test]
fn a_new_folder_in_a_watched_tree_hands_over_what_is_already_in_it() {
    let (_temp, dir) = scratch();
    let table = Table::of_tree(&dir);
    let real = dir.canonicalize().unwrap();
    let core = real.join("prompts").join("core");
    fs::create_dir_all(core.join("facts")).unwrap();
    fs::write(core.join("local-paths-rule.txt"), "Mine.\n").unwrap();
    fs::write(core.join("facts").join("env.txt"), "Mine.\n").unwrap();
    fs::write(core.join(".rule.swp"), "").unwrap();
    let mut wanted = vec![
        dir.join("prompts")
            .join("core")
            .join("facts")
            .join("env.txt"),
        dir.join("prompts")
            .join("core")
            .join("local-paths-rule.txt"),
    ];
    wanted.sort();
    for kind in [
        EventKind::Create(CreateKind::Folder),
        EventKind::Create(CreateKind::Any),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)),
    ] {
        let mut touched = table.touched(&event(kind, &real.join("prompts")));
        touched.sort();
        assert_eq!(touched, wanted, "{kind:?}");
    }
    assert!(
        table
            .touched(&event(EventKind::Modify(ModifyKind::Any), &core))
            .is_empty(),
        "还在的目录别的变动不算"
    );
}

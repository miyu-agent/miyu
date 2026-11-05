//! 问父进程（施工 XG-1 下）：起一个子进程，问出来的父进程是测试进程自己。

/// 起一个一会儿就走的子进程。
fn child() -> std::process::Child {
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/c", "ping -n 5 127.0.0.1 >NUL"]);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("sleep");
        command.arg("5");
        command
    };
    command.spawn().expect("起得来")
}

/// 收掉子进程：已经走了的不管。
#[expect(clippy::let_underscore_must_use, reason = "它可能已经自己走了")]
fn stop(mut child: std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(target_os = "macos")]
#[test]
fn the_parent_of_a_child_is_this_process() {
    let child = child();
    let parent = super::parent_process(child.id());
    stop(child);
    assert_eq!(parent, Some(std::process::id()));
    assert_eq!(
        super::parent_process(std::process::id()),
        Some(std::os::unix::process::parent_id())
    );
    // 没有这个进程的问不出。
    assert_eq!(super::parent_process(u32::MAX), None);
}

#[cfg(windows)]
#[test]
fn the_snapshot_says_a_child_belongs_to_this_process() {
    let child = child();
    let parents = super::process_parents();
    let id = child.id();
    stop(child);
    let parents = parents.expect("拍得成");
    assert_eq!(parents.get(&id), Some(&std::process::id()));
    assert!(parents.contains_key(&std::process::id()));
}

//! 藏起来的那一格（施工 5-13）：数据根就是旧版的 `~/.miyu` 这种和私密目录重了的，只写一次。

use miyu_kernel::event::Level;

use super::*;

#[test]
fn a_data_root_that_is_also_a_private_dir_is_hidden_once() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-hidden-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".miyu", ".ssh"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        (None, false),
    );
    let read_only = Permission {
        level: Level::Workspace,
        read_only: true,
    };
    let sandboxed = sandbox
        .for_call(&read_only, &scratch.to_string_lossy(), &[], &[])
        .expect("写得成")
        .expect("进沙盒");
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    assert_eq!(sandboxed.spec.hidden, [real(".miyu"), real(".ssh")]);
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 能写的里面的私密目录（施工 5-16）：能用挂载时照藏，不能时照人给的算（施工 5-13）。
#[test]
fn a_private_dir_inside_a_writable_place_is_hidden_only_with_mounts() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-mounts-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".miyu", ".ssh"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        (None, false),
    );
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    let write = [std::fs::canonicalize(&home).expect("在")];
    assert_eq!(
        sandbox.hidden(real(".miyu"), &write, false),
        [real(".miyu")],
        "没有挂载：落在能写的里面的私密目录不藏"
    );
    assert_eq!(
        sandbox.hidden(real(".miyu"), &write, true),
        [real(".miyu"), real(".ssh")],
        "有挂载：照藏"
    );
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 这一轮的私密目录（施工 5-15 下）：换上了的照它藏，出厂的去掉了的不藏、人加的藏。
#[test]
fn the_turn_private_dirs_are_what_gets_hidden() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-turn-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".miyu", ".ssh", "diary"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let mut sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        (None, false),
    );
    sandbox.set_private(vec![home.join("diary")]);
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    assert_eq!(
        sandbox.hidden(real(".miyu"), &[], false),
        [real(".miyu"), real("diary")],
        "数据根照藏，.ssh 去掉了不藏，人加的藏"
    );
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 沙盒强度（施工 5-16 下）：只用 Landlock 的那一档有挂载也不用；没设的照这台机器能不能用。
#[test]
fn weak_strength_turns_mounts_off() {
    let mut sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        None,
        PathBuf::from("data"),
        (None, false),
    );
    assert!(sandbox.mount(true));
    assert!(!sandbox.mount(false));
    sandbox.set_weak(true);
    assert!(!sandbox.mount(true), "弱沙盒不用挂载");
}

/// 写规格时照这一轮的强度（施工 5-16 下）：机器能用挂载，弱沙盒的规格也不写 `mount`。这个进程里的开关临时设上，测完放回去：
/// 这个包的别的测试不看规格的 `mount`。
#[test]
fn a_weak_turn_writes_no_mount_into_the_spec() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-weak-{}", std::process::id()));
    let home = scratch.join("home");
    std::fs::create_dir_all(home.join(".miyu")).expect("建得了目录");
    let mut sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        (None, false),
    );
    let workspace = Permission {
        level: Level::Workspace,
        read_only: false,
    };
    let cwd = scratch.to_string_lossy().into_owned();
    miyu_sandbox::set_mounts(true);
    let spec = |sandbox: &Sandbox| {
        sandbox
            .for_call(&workspace, &cwd, &[], &[])
            .expect("写得成")
            .expect("进沙盒")
            .spec
            .mount
    };
    let strong = spec(&sandbox);
    sandbox.set_weak(true);
    let weak = spec(&sandbox);
    miyu_sandbox::set_mounts(false);
    std::fs::remove_dir_all(&scratch).expect("删得掉");
    assert!(strong, "能用挂载、没设弱的写 mount");
    assert!(!weak, "弱沙盒不写");
}

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

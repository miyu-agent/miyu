//! 只读的规格（施工 RO-2）：哪儿都不能写，什么都不藏，数据根、开着的技能文件夹都一样；完全放开不进沙盒。

use miyu_kernel::event::Level;

use super::*;

/// 只读：数据根、家目录里的 `.ssh` 都不藏，开着的技能文件夹也不用另放开；完全放开不带规格。
#[test]
fn read_only_hides_nothing_and_full_access_has_no_spec() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-hidden-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".miyu/skills/a", ".ssh"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        false,
    )
    .with_shelves(vec![home.join(".miyu/skills/a")]);
    let permission = |read_only| Permission {
        level: Level::Full,
        read_only,
    };
    let cwd = scratch.to_string_lossy();
    let sandboxed = sandbox
        .for_call(&permission(true), &cwd, &[])
        .expect("写得成")
        .expect("进沙盒");
    let full = sandbox
        .for_call(&permission(false), &cwd, &[])
        .expect("写得成");
    std::fs::remove_dir_all(&scratch).expect("删得掉");
    assert_eq!(
        sandboxed.spec,
        Spec {
            write: Vec::new(),
            hidden: Vec::new(),
            read: None,
            reveal: Vec::new(),
            sockets: Vec::new(),
        }
    );
    assert_eq!(full, None);
}

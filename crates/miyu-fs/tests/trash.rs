//! 系统的回收站（施工 4-6 下）：放进去，交回它在回收站里的位置。Linux 上回收站在场地的假家目录里，记下了
//! `.trashinfo`；macOS、Windows 上放进系统真的回收站（在 CI 上跑），看完从回收站里删掉。施工 U-1 起撤销不再移回来。

use std::fs;
use std::path::Path;

use miyu_fs::trash::put;

use crate::support::Scratch;

/// 把 `real` 放进回收站，交回它在回收站里的位置。
fn kept(real: &Path, home: Option<&Path>) -> String {
    put(real, home).unwrap_or_else(|refused| panic!("{}: {refused:?}", real.display()))
}

#[cfg(target_os = "linux")]
#[test]
fn a_file_goes_into_the_home_trash_with_its_record() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    fs::create_dir_all(&home).expect("建得了");
    crate::support::file(&scratch.0.join("work/a.txt"), "old\n");
    let real = fs::canonicalize(scratch.0.join("work/a.txt")).expect("在");
    let kept = kept(&real, Some(&home));
    assert!(!real.exists(), "原处没了");
    assert_eq!(fs::read_to_string(&kept).expect("在回收站里"), "old\n");
    assert!(
        home.join(".local/share/Trash/info/a.txt.trashinfo")
            .is_file(),
        "放进去时记下了"
    );
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn files_and_directories_go_into_the_system_trash() {
    let scratch = Scratch::new();
    crate::support::file(&scratch.0.join("work/a.txt"), "old\n");
    crate::support::file(&scratch.0.join("work/dir/inner.txt"), "in\n");
    for name in ["a.txt", "dir"] {
        let real = fs::canonicalize(scratch.0.join("work").join(name)).expect("在");
        let kept = kept(&real, None);
        let kept = Path::new(&kept);
        assert!(!real.exists(), "{name} 进了回收站");
        assert!(kept.exists(), "{name}：回收站里有");
        // 看完从系统的回收站里删掉，不给 CI 机器留东西；Windows 上连 `$I` 那份记录。
        match kept.is_dir() {
            true => fs::remove_dir_all(kept).expect("删得掉"),
            false => fs::remove_file(kept).expect("删得掉"),
        }
        #[cfg(windows)]
        if let Some(rest) = kept
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("$R"))
            && let Err(error) = fs::remove_file(kept.with_file_name(format!("$I{rest}")))
        {
            eprintln!("{name}：$I 记录没删掉：{error}");
        }
    }
}

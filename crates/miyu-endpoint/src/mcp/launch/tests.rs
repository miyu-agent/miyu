use std::path::{Path, PathBuf};

use super::*;

/// 命令第一项：绝对路径的照它；带路径分隔符的照包目录算；只有名字的先找包目录的 `bin/`，没有的交回名字本身（照 `PATH` 找）。
#[test]
fn the_program_is_found_in_the_package_or_left_to_the_path() {
    let dir = std::env::temp_dir().join(format!("miyu-mcp-launch-{}", std::process::id()));
    let bin = dir.join(BIN);
    std::fs::create_dir_all(&bin).unwrap();
    let bundled = bin.join(format!("server{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&bundled, b"").unwrap();

    let absolute = std::env::temp_dir().join("elsewhere").join("server");
    assert_eq!(program(&absolute.to_string_lossy(), &dir), absolute);
    assert_eq!(program("scripts/run", &dir), dir.join("scripts/run"));
    assert_eq!(program("server", &dir), bundled, "包带的放在 bin/");
    assert_eq!(
        program("npx", &dir),
        PathBuf::from("npx"),
        "包里没有的照 PATH 找"
    );
    assert_eq!(
        program("server", Path::new("relative")),
        PathBuf::from("server"),
        "包目录不是绝对路径的不找"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

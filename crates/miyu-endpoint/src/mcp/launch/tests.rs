use std::path::{Path, PathBuf};

use super::*;

/// 命令第一项：绝对路径的照它；带路径分隔符的照包目录算；只有名字的先找包目录的 `bin/`，都没有的交回名字本身。
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
    // 哪儿都没有的名字（`npx` 不行：Windows 上装了 Node 的会找成 `npx.cmd`，施工 X-5）。
    assert_eq!(
        program("miyu-no-such-program", &dir),
        PathBuf::from("miyu-no-such-program"),
        "包里、PATH 里都没有的交回名字本身"
    );
    assert_eq!(
        program("server", Path::new("relative")),
        PathBuf::from("server"),
        "包目录不是绝对路径的不找"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Windows 上的找法（施工 X-5）：`.exe`、`.cmd`、`.bat` 照先后，包目录的 `bin/` 在 `PATH` 前面；找到的交完整路径，都没有的交回
/// 名字本身。只是照文件名找，哪个平台上都试得了。
#[test]
fn on_windows_a_cmd_wrapper_is_found_in_the_package_and_on_the_path() {
    let root = std::env::temp_dir().join(format!("miyu-mcp-suffix-{}", std::process::id()));
    let (dir, path) = (root.join("pkg"), root.join("path"));
    let bin = dir.join(BIN);
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    for file in [
        bin.join("own.cmd"),
        path.join("npx.cmd"),
        path.join("uvx.exe"),
        path.join("uvx.cmd"),
        bin.join("both.bat"),
        path.join("both.exe"),
    ] {
        std::fs::write(file, b"").unwrap();
    }
    let windows = [".exe", ".cmd", ".bat"];
    let dirs = [path.clone()];
    assert_eq!(find("own", &dir, &dirs, &windows), bin.join("own.cmd"));
    assert_eq!(find("npx", &dir, &dirs, &windows), path.join("npx.cmd"));
    assert_eq!(
        find("uvx", &dir, &dirs, &windows),
        path.join("uvx.exe"),
        ".exe 在前"
    );
    assert_eq!(
        find("both", &dir, &dirs, &windows),
        bin.join("both.bat"),
        "包里的在前"
    );
    assert_eq!(find("node", &dir, &dirs, &windows), PathBuf::from("node"));
    assert_eq!(
        find("npx", &dir, &dirs, &[""]),
        PathBuf::from("npx"),
        "别的平台不认 .cmd"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

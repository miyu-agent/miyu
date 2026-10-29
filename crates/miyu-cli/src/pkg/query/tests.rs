//! `files`、`owns`、`check` 印成的那几行（施工 F-8 下，`docs/blueprint/cli/pkg.md`「样子」）。

use serde_json::json;

use super::{checked, files, owner, size};
use crate::language::Language;

#[test]
fn files_are_the_id_and_the_whole_path() {
    let dir = std::env::temp_dir().join("xpkg");
    let listed = json!({"dir": dir, "files": [{"path": "bin/a.txt"}, {"path": "package.toml"}]});
    assert_eq!(
        files("xpkg", &listed),
        [
            format!("xpkg {}", dir.join("bin").join("a.txt").display()),
            format!("xpkg {}", dir.join("package.toml").display()),
        ]
    );
}

#[test]
fn owns_tells_installed_from_written_later() {
    let zh = Language::Chinese;
    let recorded = json!({"package": "xpkg", "layer": "home", "path": "a", "recorded": true});
    assert_eq!(
        owner("/p/a", &recorded, &zh).as_deref(),
        Some("/p/a 属于 xpkg")
    );
    let later = json!({"package": "xpkg", "layer": "home", "path": "c", "recorded": false});
    assert_eq!(
        owner("/p/c", &later, &zh).as_deref(),
        Some("/p/c 在 xpkg 的目录里，不是安装时带的")
    );
    let shipped = json!({"package": "net", "layer": "shipped", "path": "a", "recorded": false});
    assert_eq!(
        owner("/r/a", &shipped, &zh).as_deref(),
        Some("/r/a 属于 net")
    );
    assert_eq!(owner("/x", &json!({"package": null}), &zh), None);
}

#[test]
fn check_lists_what_changed_and_only_changes_and_losses_fail() {
    let zh = Language::Chinese;
    let found = json!({"packages": [
        {"package": "a", "modified": ["x"], "missing": ["y"], "extra": ["z"]},
        {"package": "b", "modified": [], "missing": [], "extra": []},
    ]});
    assert_eq!(
        checked(&found, &zh),
        (
            vec![
                "a：已修改 x".to_string(),
                "a：缺失 y".to_string(),
                "a：多出 z".to_string(),
                "b：正常".to_string(),
            ],
            true
        )
    );
    let extra =
        json!({"packages": [{"package": "a", "modified": [], "missing": [], "extra": ["z"]}]});
    assert_eq!(
        checked(&extra, &Language::English),
        (vec!["a: extra z".to_string()], false)
    );
    assert_eq!(
        checked(&json!({"packages": []}), &zh),
        (vec!["没有可检查的软件包".to_string()], false)
    );
}

#[test]
fn sizes_read_like_pacman() {
    assert_eq!(size(0), "0 B");
    assert_eq!(size(1023), "1023 B");
    assert_eq!(size(1024), "1.0 KiB");
    assert_eq!(size(1_572_864), "1.5 MiB");
    assert_eq!(size(5 * 1024 * 1024 * 1024 * 1024), "5120.0 GiB");
}

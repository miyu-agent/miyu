//! 只禁写的名单（施工 5-17 下）：三种写法、出厂的拼法、去掉照写法比、不收的写法和太宽的。

use super::*;

fn home() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Users\alice")
    } else {
        PathBuf::from("/home/alice")
    }
}

fn names(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

#[test]
fn an_entry_is_inside_a_workspace_under_home_or_absolute() {
    let home = home();
    assert_eq!(
        readonly_entry(Some(&home), ".git/hooks"),
        Some(ReadonlyEntry::Inside(names(&[".git", "hooks"])))
    );
    assert_eq!(
        readonly_entry(Some(&home), "./.vscode/"),
        Some(ReadonlyEntry::Inside(names(&[".vscode"]))),
        "空的、. 跳过"
    );
    assert_eq!(
        readonly_entry(Some(&home), "~/.bashrc"),
        Some(ReadonlyEntry::Path(home.join(".bashrc")))
    );
    assert_eq!(
        readonly_entry(Some(&home), "~"),
        Some(ReadonlyEntry::Path(home.clone()))
    );
    let absolute = if cfg!(windows) {
        r"D:\tools\hook"
    } else {
        "/srv/hook"
    };
    assert_eq!(
        readonly_entry(None, absolute),
        Some(ReadonlyEntry::Path(PathBuf::from(absolute))),
        "绝对路径不要家目录"
    );
    for bad in ["", ".", "./", "../x", "a/../b"] {
        assert_eq!(readonly_entry(Some(&home), bad), None, "{bad:?}");
    }
    assert_eq!(readonly_entry(None, "~/.bashrc"), None, "家目录不知道");
}

#[test]
fn the_shipped_list_starts_with_the_workspace_ones() {
    let list = Readonly::shipped(Some(&home()));
    assert_eq!(
        list.inside,
        [
            names(&[".git", "hooks"]),
            names(&[".git", "config"]),
            names(&[".miyu"]),
            names(&[".mcp.json"]),
            names(&[".vscode"]),
            names(&[".idea"]),
            names(&[".envrc"]),
        ]
    );
    assert!(list.paths.contains(&home().join(".gitconfig")));
    let here = if cfg!(target_os = "macos") {
        home().join("Library").join("LaunchAgents")
    } else if cfg!(windows) {
        home()
            .join("Documents")
            .join("PowerShell")
            .join("profile.ps1")
    } else {
        home().join(".bashrc")
    };
    assert!(list.paths.contains(&here), "{}", here.display());
    assert_eq!(
        list.inside.len() + list.paths.len(),
        SHIPPED_READONLY.len(),
        "一项不少"
    );
    let nowhere = Readonly::shipped(None);
    assert_eq!(nowhere.inside.len(), 7, "相对的照旧");
    assert!(nowhere.paths.is_empty(), "家目录不知道的没有");
}

#[test]
fn removed_ones_go_and_added_ones_come() {
    let home = home();
    let absolute = if cfg!(windows) {
        r"D:\tools\hook"
    } else {
        "/srv/hook"
    };
    let list = Readonly::with(
        Some(&home),
        &[
            "Makefile".to_string(),
            "~/bin".to_string(),
            absolute.to_string(),
            ".vscode".to_string(),
            "../out".to_string(),
            "~".to_string(),
        ],
        &[
            ".vscode".to_string(),
            "~/.gitconfig".to_string(),
            "~/not-shipped".to_string(),
        ],
    );
    assert!(list.inside.contains(&names(&["Makefile"])));
    assert_eq!(
        list.inside
            .iter()
            .filter(|entry| **entry == names(&[".vscode"]))
            .count(),
        1,
        "去掉了又加回来的照加的算，只一份"
    );
    assert!(list.paths.contains(&home.join("bin")));
    assert!(list.paths.contains(&PathBuf::from(absolute)));
    assert!(!list.paths.contains(&home.join(".gitconfig")), "去掉了");
    assert!(!list.paths.contains(&home), "家目录太宽，不收");
    assert!(
        !list
            .inside
            .iter()
            .any(|entry| entry.contains(&"..".to_string())),
        "写法不对的不收"
    );
}

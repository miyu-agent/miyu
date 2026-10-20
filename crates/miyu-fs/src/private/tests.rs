//! 出厂的清单、人加的、去掉的：拼法、去掉照写法比、不收的写法和太宽的。

use super::*;

fn home() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Users\alice")
    } else {
        PathBuf::from("/home/alice")
    }
}

#[test]
fn shipped_dirs_hang_under_home() {
    let dirs = private_dirs(&home());
    assert_eq!(dirs.len(), SHIPPED_PRIVATE.len());
    assert_eq!(dirs[0], home().join(".ssh"));
    assert_eq!(dirs[4], home().join(".config").join("gh"), "一段一段接上");
}

#[test]
fn removed_ones_go_and_added_ones_come() {
    let root = if cfg!(windows) {
        r"D:\secret"
    } else {
        "/srv/secret"
    };
    let dirs = private_dirs_with(
        &home(),
        &[
            "~/notes/diary".to_string(),
            root.to_string(),
            "relative".to_string(),
            "~/.ssh".to_string(),
        ],
        &["~/.ssh".to_string(), "~/not-shipped".to_string()],
    );
    assert!(dirs.contains(&home().join("notes").join("diary")));
    assert!(dirs.contains(&PathBuf::from(root)));
    assert!(
        dirs.contains(&home().join(".ssh")),
        "去掉了又自己加回来的照加的"
    );
    assert_eq!(
        dirs.iter()
            .filter(|dir| **dir == home().join(".ssh"))
            .count(),
        1,
        "不重复"
    );
    assert_eq!(dirs.len(), SHIPPED_PRIVATE.len() + 2, "相对路径不算");
    let gone = private_dirs_with(&home(), &[], &["~/.gnupg".to_string()]);
    assert!(!gone.contains(&home().join(".gnupg")));
}

#[test]
fn home_and_above_are_too_broad() {
    assert!(too_broad(&home(), &home()));
    assert!(too_broad(&home(), home().parent().expect("有上级")));
    assert!(!too_broad(&home(), &home().join(".ssh")));
    let dirs = private_dirs_with(&home(), &["~/".to_string()], &[]);
    assert_eq!(dirs.len(), SHIPPED_PRIVATE.len(), "家目录本身不算");
    if !cfg!(windows) {
        assert!(too_broad(&home(), Path::new("/")));
        assert!(
            !too_broad(&home(), Path::new("/home/alicex")),
            "照一段一段比"
        );
    }
}

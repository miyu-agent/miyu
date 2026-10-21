//! 出厂的清单、人加的、去掉的：拼法、去掉照写法比、不收的写法和太宽的。

use std::ffi::OsString;

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
        &|_| None,
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
    let gone = private_dirs_with(&home(), &[], &["~/.gnupg".to_string()], &|_| None);
    assert!(!gone.contains(&home().join(".gnupg")));
}

#[test]
fn home_and_above_are_too_broad() {
    assert!(too_broad(&home(), &home()));
    assert!(too_broad(&home(), home().parent().expect("有上级")));
    assert!(!too_broad(&home(), &home().join(".ssh")));
    assert_eq!(
        written(&home(), "~"),
        Some(home()),
        "只写 ~ 也是家目录（施工 5-15 下补）"
    );
    let dirs = private_dirs_with(&home(), &["~/".to_string(), "~".to_string()], &[], &|_| {
        None
    });
    assert_eq!(dirs.len(), SHIPPED_PRIVATE.len(), "家目录本身不算");
    if !cfg!(windows) {
        assert!(too_broad(&home(), Path::new("/")));
        assert!(
            !too_broad(&home(), Path::new("/home/alicex")),
            "照一段一段比"
        );
    }
}

/// 这个系统上的一个绝对路径：Unix 上 `/x/…`，Windows 上 `D:\x\…`。
fn abs(rest: &str) -> PathBuf {
    let base = if cfg!(windows) { r"D:\" } else { "/" };
    rest.split('/')
        .fold(PathBuf::from(base), |dir, part| dir.join(part))
}

/// 跟着环境变量挪的（施工 5-17 上）：新位置接在出厂项后面；几个路径的照系统的分隔符分开；目录下的凭据文件接上文件名；
/// `~/.config/` 下的照 `XDG_CONFIG_HOME` 挪；出厂项去掉了的，挪去的也不藏；不是绝对路径的、家目录和它的上级不算。
#[test]
fn shipped_ones_follow_their_environment_variables() {
    let kube = std::env::join_paths([abs("k/one"), abs("k/two")]).expect("拼得成");
    let home_text = home().into_os_string();
    let env = move |name: &str| -> Option<OsString> {
        match name {
            "GNUPGHOME" => Some(abs("vault/gpg").into_os_string()),
            "KUBECONFIG" => Some(kube.clone()),
            "CARGO_HOME" => Some(abs("tools/cargo").into_os_string()),
            "XDG_CONFIG_HOME" => Some(abs("cfg").into_os_string()),
            "DOCKER_CONFIG" => Some(OsString::from("relative/docker")),
            "AZURE_CONFIG_DIR" => Some(home_text.clone()),
            _ => None,
        }
    };
    let dirs = private_dirs_with(&home(), &[], &[], &env);
    assert!(dirs.contains(&home().join(".gnupg")), "原来的照旧在");
    assert!(dirs.contains(&abs("vault/gpg")));
    assert!(dirs.contains(&abs("k/one")) && dirs.contains(&abs("k/two")));
    assert!(dirs.contains(&abs("tools/cargo").join("credentials.toml")));
    assert!(
        dirs.contains(&abs("cfg").join("gh")),
        "~/.config/ 下的照 XDG_CONFIG_HOME"
    );
    assert!(
        !dirs.contains(&PathBuf::from("relative/docker")),
        "相对路径不算"
    );
    assert!(
        dirs.iter().all(|dir| !too_broad(&home(), dir)),
        "家目录本身不算：{dirs:?}"
    );
    let removed = private_dirs_with(&home(), &[], &["~/.gnupg".to_string()], &env);
    assert!(
        !removed.contains(&abs("vault/gpg")),
        "去掉了出厂项，挪去的也不藏"
    );
}

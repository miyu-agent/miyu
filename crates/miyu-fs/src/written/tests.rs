//! 人写的目录：`~/…` 一段一段照家目录接上，绝对路径照原样，相对路径不收。

use super::*;

fn home() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Users\alice")
    } else {
        PathBuf::from("/home/alice")
    }
}

#[test]
fn a_tilde_hangs_under_home_and_an_absolute_path_stays() {
    assert_eq!(written(&home(), "~"), Some(home()));
    assert_eq!(
        written(&home(), "~/.agents//skills/"),
        Some(home().join(".agents").join("skills")),
        "一段一段接上"
    );
    let root = if cfg!(windows) {
        r"D:\skills"
    } else {
        "/srv/skills"
    };
    assert_eq!(written(&home(), root), Some(PathBuf::from(root)));
    assert_eq!(written(&home(), "relative/dir"), None);
}

/// `~\` 和文件工具一样认（施工 WN-1，照 `tilde`）：Windows 上是家目录下面，别处 `\` 不是分隔符，照旧不收。
#[test]
fn a_backslash_after_the_tilde_follows_the_platform() {
    let written = written(&home(), r"~\.config\gh");
    if cfg!(windows) {
        assert_eq!(written, Some(home().join(".config").join("gh")));
    } else {
        assert_eq!(written, None);
    }
}

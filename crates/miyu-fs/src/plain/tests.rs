use std::path::{Path, PathBuf};

use super::{handed, plain, usable};

#[test]
fn the_verbatim_prefix_is_dropped_and_unc_comes_back_as_two_slashes() {
    assert_eq!(
        plain(Path::new(r"\\?\C:\Users\me\.miyu\workspace")),
        r"C:\Users\me\.miyu\workspace"
    );
    assert_eq!(plain(Path::new(r"\\?\UNC\host\share\a")), r"\\host\share\a");
    assert_eq!(plain(Path::new(r"C:\plain")), r"C:\plain");
    assert_eq!(plain(Path::new("/home/u/a.rs")), "/home/u/a.rs");
}

#[test]
fn a_path_plain_windows_can_read_the_same_way_loses_the_prefix() {
    let got = |text: &str| usable(Path::new(text)).to_string_lossy().into_owned();
    assert_eq!(
        got(r"\\?\C:\mt\home\.miyu\var\lib\kit"),
        r"C:\mt\home\.miyu\var\lib\kit"
    );
    assert_eq!(got(r"\\?\UNC\host\share\a"), r"\\host\share\a");
    // 普通写法会被系统改义的照原样：保留名、名字结尾是点或空格、超过 259 个字。
    for kept in [
        r"\\?\C:\x\con",
        r"\\?\C:\x\NUL.txt",
        r"\\?\C:\x\com1",
        r"\\?\C:\x\lpt9.log",
        r"\\?\C:\x\dots.",
        r"\\?\C:\x\space ",
    ] {
        assert_eq!(got(kept), kept, "{kept}");
    }
    let long = format!(r"\\?\C:\{}", "a".repeat(260));
    assert_eq!(got(&long), long, "超长的照原样");
    // 没有前缀的、别的平台的照原样。
    assert_eq!(got(r"C:\plain"), r"C:\plain");
    assert_eq!(got("/home/u/a.rs"), "/home/u/a.rs");
    assert_eq!(
        got(r"\\?\C:\x\console"),
        r"C:\x\console",
        "保留名只认整个名字"
    );
}

/// 一个用完就删的临时目录，换成真实位置（macOS 的临时目录在链接 `/var` 下）。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("miyu-handed-{}-{name}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).expect("建得了目录");
        Scratch(std::fs::canonicalize(dir).expect("换得了真实位置"))
    }

    /// 临时目录（`more` 空的）或者它底下 `more` 的真实位置，写成交出去的样子。
    fn text(&self, more: &str) -> String {
        let path = match more {
            "" => self.0.clone(),
            more => self.0.join(more),
        };
        usable(&path).to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 施工 XW-1：从旧版导入的会话记的工作目录是 `~`，扩展拿字面的 `~` 换不成真实位置，发什么文件都被拒。
#[test]
fn a_home_relative_workspace_is_handed_out_as_a_real_path() {
    let home = Scratch::new("tilde");
    assert_eq!(handed("~", Some(&home.0)), home.text(""));
    assert_eq!(handed("~/src", Some(&home.0)), home.text("src"));
    let absolute = home.text("src");
    assert_eq!(handed(&absolute, None), absolute);
}

#[test]
fn a_workspace_that_cannot_be_resolved_is_handed_out_as_written() {
    assert_eq!(handed("~", None), "~");
    assert_eq!(handed("~/src", None), "~/src");
}

use std::path::Path;

use super::{plain, usable};

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

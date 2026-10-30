//! 相对路径照字面截（施工 V-31）：交回的和一段一段比、一段一段拼的（`slow`）一字不差；截不了的交给它。

use std::path::{MAIN_SEPARATOR_STR, Path, PathBuf};

use super::super::{quick_root, relative, slow};

/// 照两条路各算一遍，要一样；`expected` 是交回的样子。
fn same(root: &str, path: &str, expected: Option<&str>) {
    let (root, path) = (Path::new(root), Path::new(path));
    let got = relative(root, quick_root(root), path);
    assert_eq!(got, slow(root, path), "起点 {root:?}，路径 {path:?}");
    assert_eq!(got.as_deref(), expected, "起点 {root:?}，路径 {path:?}");
}

/// 往下走目录时走到的样子：起点一层一层 `join` 名字。
#[test]
fn a_joined_path_is_cut_by_the_length_of_the_root() {
    let root = std::env::temp_dir().join("miyu-relative");
    let text = root.to_string_lossy().into_owned();
    assert!(quick_root(&root).is_some(), "临时目录走快的那条路");
    for (parts, expected) in [
        (vec!["a.rs"], "a.rs"),
        (vec!["src", "deep", "b.rs"], "src/deep/b.rs"),
        (vec![".hidden", "字", "😀 x.txt"], ".hidden/字/😀 x.txt"),
        (vec!["..."], "..."),
    ] {
        let path: PathBuf = parts.iter().fold(root.clone(), |dir, part| dir.join(part));
        same(&text, &path.to_string_lossy(), Some(expected));
    }
    same(&text, &text, Some(""));
}

/// 怪的写法：截不了的照一段段比，交回的照旧。
#[test]
fn odd_paths_read_the_same_as_before() {
    let s = MAIN_SEPARATOR_STR;
    let base = std::env::temp_dir().to_string_lossy().into_owned();
    let base = base.trim_end_matches(s);
    let cases = [
        // 起点以分隔符结尾。
        (format!("{base}{s}r{s}"), format!("{base}{s}r{s}a{s}b")),
        // 中间多一个分隔符、夹着 `.`、结尾多一个分隔符。
        (format!("{base}{s}r"), format!("{base}{s}r{s}a{s}{s}b")),
        (format!("{base}{s}r"), format!("{base}{s}r{s}.{s}a")),
        (format!("{base}{s}r"), format!("{base}{s}r{s}a{s}")),
        (format!("{base}{s}r"), format!("{base}{s}r{s}{s}a")),
        // 字面是前缀、不是一段段的前缀；不是前缀。
        (format!("{base}{s}r"), format!("{base}{s}rr{s}a")),
        (format!("{base}{s}r"), format!("{base}{s}x{s}a")),
        // 起点写了 `.`、`..`。
        (format!("{base}{s}.{s}r"), format!("{base}{s}.{s}r{s}a")),
        (format!("{base}{s}r{s}."), format!("{base}{s}r{s}.{s}a")),
        (format!("{base}{s}r{s}.."), format!("{base}{s}r{s}..{s}a")),
        (format!("{base}{s}r{s}"), format!("{base}{s}r{s}{s}a")),
        // 空的起点、相对的起点。
        (String::new(), format!("{base}{s}a")),
        (String::new(), "a".to_owned()),
        ("r".to_owned(), format!("r{s}a{s}b")),
        (".".to_owned(), format!(".{s}a")),
    ];
    for (root, path) in &cases {
        let (root, path) = (Path::new(root), Path::new(path));
        assert_eq!(
            relative(root, quick_root(root), path),
            slow(root, path),
            "起点 {root:?}，路径 {path:?}"
        );
    }
}

/// 根目录本身是起点。
#[cfg(unix)]
#[test]
fn from_the_unix_root() {
    same("/", "/a/b", Some("a/b"));
    same("/", "/", Some(""));
    same("/x", "/x/a\\b", Some("a\\b"));
}

/// 不是 UTF-8 的名字：照一段段比，坏字节各换成 `�`。
#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf8_reads_the_same() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let root = Path::new("/tmp/r");
    let path = root.join(OsStr::from_bytes(b"a\xE2/b\xFF"));
    assert_eq!(
        relative(root, quick_root(root), &path).as_deref(),
        Some("a\u{FFFD}/b\u{FFFD}")
    );
    let odd = Path::new(OsStr::from_bytes(b"/tmp/\xFF"));
    assert_eq!(quick_root(odd), None);
    assert_eq!(
        relative(odd, quick_root(odd), &odd.join("a")),
        slow(odd, &odd.join("a"))
    );
}

/// Windows 的几种写法：盘符、`\\?\`、UNC、`/` 当分隔符、只有前缀没有根。
#[cfg(windows)]
#[test]
fn windows_spellings_read_the_same() {
    same(r"C:\", r"C:\a\b", Some("a/b"));
    same(r"C:\r", r"C:\r\a\b", Some("a/b"));
    same(r"\\?\C:\r", r"\\?\C:\r\a\b", Some("a/b"));
    same(r"\\?\C:\", r"\\?\C:\a", Some("a"));
    same(r"\\server\share\r", r"\\server\share\r\a", Some("a"));
    same(
        r"\\?\UNC\server\share\r",
        r"\\?\UNC\server\share\r\a",
        Some("a"),
    );
    same("C:/r", r"C:/r\a\b", Some("a/b"));
    for (root, path) in [
        (r"C:", r"C:\a"),
        (r"C:", r"C:a"),
        (r"\\?\C:", r"\\?\C:\a"),
        (r"\\server", r"\\server\share\a"),
        (r"\\?\C:\r", r"\\?\C:\r\a/b"),
        (r"C:\r", r"C:\r/a"),
        ("C:/r/", "C:/r/a"),
        (r"c:\r", r"C:\r\a"),
    ] {
        let (root, path) = (Path::new(root), Path::new(path));
        assert_eq!(
            relative(root, quick_root(root), path),
            slow(root, path),
            "起点 {root:?}，路径 {path:?}"
        );
    }
}

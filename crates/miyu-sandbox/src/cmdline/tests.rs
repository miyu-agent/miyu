//! 照 `CommandLineToArgvW` 的规矩拼：照写的原样、要引号的、引号里的双引号和反斜杠、结尾的反斜杠、空的参数。

use std::ffi::OsStr;

use super::*;

fn line(program: &str, args: &[&str]) -> String {
    windows_command_line(OsStr::new(program), args).expect("都是合法的 Unicode")
}

#[test]
fn plain_words_stay_as_they_are() {
    assert_eq!(
        line(r"C:\git\git.exe", &["--version"]),
        r"C:\git\git.exe --version"
    );
    assert_eq!(
        line("cmd", &["/c", r"echo\hi"]),
        r"cmd /c echo\hi",
        "不挨着双引号的反斜杠不动"
    );
}

#[test]
fn spaces_quotes_and_backslashes_are_escaped() {
    assert_eq!(
        line(r"C:\Program Files\x.exe", &[]),
        r#""C:\Program Files\x.exe""#
    );
    assert_eq!(line("p", &[""]), r#"p """#, "空的参数照样占一个");
    assert_eq!(line("p", &[r#"say "hi""#]), r#"p "say \"hi\"""#);
    assert_eq!(
        line("p", &[r#"a\"b"#]),
        r#"p "a\\\"b""#,
        "双引号前的反斜杠成倍"
    );
    assert_eq!(
        line("p", &[r"C:\dir with space\"]),
        r#"p "C:\dir with space\\""#,
        "结尾的反斜杠成倍"
    );
    assert_eq!(line("p", &["tab\there"]), "p \"tab\there\"");
}

//! 校验和清单：`sha256sum` 写的样子收，写坏的每一种都拒、说第几行。

use super::*;

const A: &str = "627032f78fc3332efe2e138b2a0c8d6e5e37c02f5a5ac3db7315b254fe8d7ec9";
const B: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[test]
fn what_sha256sum_writes_is_fine() {
    let text = format!("{A}  miyu-1.0.0-linux-x86_64.tar.gz\n{B}  miyu-1.0.0-windows-x86_64.zip\n");
    assert_eq!(check(&text), Ok(2));
}

#[test]
fn each_mistake_is_refused_with_its_line() {
    let cases = [
        (String::new(), "empty"),
        (format!("{A}  a.tar.gz"), "newline"),
        (format!("{A} a.tar.gz\n"), "line 1: not"),
        (
            format!("{A}  a.tar.gz\n{}  b.zip\n", &A[..63]),
            "line 2: the hash",
        ),
        (
            format!("{}  a.tar.gz\n", A.to_uppercase()),
            "line 1: the hash",
        ),
        (format!("{A}  dir/a.tar.gz\n"), "not a plain file name"),
        (format!("{A}  dir\\a.zip\n"), "not a plain file name"),
        (format!("{A}  ..\n"), "not a plain file name"),
        (format!("{A}  \n"), "not a plain file name"),
        (format!("{A}   a.tar.gz\n"), "not a plain file name"),
        (format!("{A}  a\tb\n"), "not a plain file name"),
        (
            format!("{A}  a.tar.gz\n{B}  a.tar.gz\n"),
            "line 2: `a.tar.gz` is listed twice",
        ),
    ];
    for (text, said) in cases {
        let error = check(&text).expect_err(&text);
        assert!(error.contains(said), "{text:?} → {error}");
    }
}

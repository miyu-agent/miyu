//! 作者、上游（施工 F-12）：写了的读出来，没写的没有；作者空的、太长、两行的，上游不是 `https://` 的、带空白的报错。

use crate::package::{Code, read};

/// 一个界面包，`[package]` 里再接上 `extra`。
fn ui(extra: &str) -> String {
    format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"X\" }}\n{extra}\n\n[ui]\n")
}

#[test]
fn credits_are_read_when_written() {
    let manifest = read(&ui(
        "author = \" miyu-agent \"\nupstream = \"https://github.com/miyu-agent/miyu\"",
    ))
    .unwrap();
    assert_eq!(
        manifest.author.as_deref(),
        Some("miyu-agent"),
        "去掉前后空白"
    );
    assert_eq!(
        manifest.upstream.as_deref(),
        Some("https://github.com/miyu-agent/miyu")
    );
    let bare = read(&ui("")).unwrap();
    assert_eq!((bare.author, bare.upstream), (None, None));
}

#[test]
fn wrong_credits_are_refused() {
    let wrong = |extra: &str| read(&ui(extra)).expect_err("应该读不成").code;
    assert_eq!(wrong("author = \"  \""), Code::BadAuthor);
    assert_eq!(wrong("author = 1"), Code::BadAuthor);
    assert_eq!(wrong("author = \"a\\nb\""), Code::BadAuthor);
    assert_eq!(
        wrong(&format!("author = \"{}\"", "a".repeat(101))),
        Code::BadAuthor
    );
    assert_eq!(
        wrong("upstream = \"http://github.com/x\""),
        Code::BadUpstream
    );
    assert_eq!(wrong("upstream = \"https://\""), Code::BadUpstream);
    assert_eq!(wrong("upstream = \"https://a b\""), Code::BadUpstream);
    assert_eq!(wrong("upstream = 3"), Code::BadUpstream);
}

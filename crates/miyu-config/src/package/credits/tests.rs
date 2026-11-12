//! 作者、上游（施工 F-12）、许可证（施工 PV-3）：写了的读出来，没写的没有；作者、许可证空的、太长、两行的、不是字的，上游不是
//! `https://` 的、带空白的报错。

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

#[test]
fn a_license_is_read_as_written_and_is_optional() {
    let manifest = read(&ui("license = \"  MIT OR Apache-2.0 \"")).unwrap();
    assert_eq!(
        manifest.license.as_deref(),
        Some("MIT OR Apache-2.0"),
        "去掉前后空白，别的照原样"
    );
    let any_text = read(&ui("license = \"自己写的许可\"")).unwrap();
    assert_eq!(
        any_text.license.as_deref(),
        Some("自己写的许可"),
        "不查是不是合法的 SPDX"
    );
    let at_most = "a".repeat(100);
    let longest = read(&ui(&format!("license = \"{at_most}\""))).unwrap();
    assert_eq!(longest.license.as_deref(), Some(at_most.as_str()));
    assert_eq!(read(&ui("")).unwrap().license, None);
}

#[test]
fn a_wrong_license_is_refused() {
    let wrong = |extra: &str| read(&ui(extra)).expect_err("应该读不成");
    for extra in [
        "license = \"   \"".to_string(),
        "license = \"\"".to_string(),
        "license = 1".to_string(),
        "license = [\"MIT\"]".to_string(),
        "license = \"MIT\\nApache-2.0\"".to_string(),
        format!("license = \"{}\"", "a".repeat(101)),
    ] {
        let problem = wrong(&extra);
        assert_eq!(problem.code, Code::BadLicense, "{extra}");
        assert_eq!(problem.code.as_str(), "bad_license");
        assert_eq!(problem.detail, "package.license");
        assert_eq!(problem.line, Some(4), "报在写的那一行");
    }
}

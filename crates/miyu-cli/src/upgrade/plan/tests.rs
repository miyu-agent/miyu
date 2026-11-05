//! 标记、版本、挑发布、压缩包名、清单里的那一行。

use serde_json::json;

use super::*;

fn v(text: &str) -> Version {
    Version::parse(text).expect(text)
}

#[test]
fn the_marker_names_who_installed_it() {
    assert_eq!(channel("channel = \"script\"\n"), Ok(Channel::Script));
    assert_eq!(
        channel("# 安装脚本写的\nchannel = \"aur\"  # 包管理器\n"),
        Ok(Channel::Aur)
    );
    assert_eq!(channel("version = 1\nchannel=\"brew\""), Ok(Channel::Brew));
    for (text, said) in [
        ("", "no channel"),
        ("other = \"x\"\n", "no channel"),
        ("channel = \"apt\"\n", "unknown channel `apt`"),
        ("channel = script\n", "not a quoted string"),
        ("channel\n", "not `key = \"value\"`"),
    ] {
        let error = channel(text).expect_err(text);
        assert!(error.contains(said), "{text:?} → {error}");
    }
}

#[test]
fn versions_read_and_print_back() {
    for text in ["0.0.0", "1.0.0", "1.2.30", "1.0.0-rc.1", "2.0.0-beta.2.x"] {
        assert_eq!(v(text).to_string(), text);
    }
    for bad in [
        "",
        "1",
        "1.0",
        "1.0.0.0",
        "1.0.x",
        "v1.0.0",
        "1.0.0-",
        "1.0.0-rc..1",
        "1.0.0-rc_1",
        "1..0",
    ] {
        assert!(Version::parse(bad).is_none(), "{bad}");
    }
    assert!(v("1.0.0-rc.1").is_pre());
    assert!(!v("1.0.0").is_pre());
}

#[test]
fn versions_order_like_semver() {
    let ordered = [
        "0.0.0",
        "0.0.1",
        "0.1.0",
        "0.9.9",
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-rc.1",
        "1.0.0-rc.2",
        "1.0.0-rc.10",
        "1.0.0",
        "1.0.1",
        "1.10.0",
        "2.0.0",
    ];
    for pair in ordered.windows(2) {
        assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
    }
    assert_eq!(v("1.0.0").cmp(&v("1.0.0")), Ordering::Equal);
    assert!(v("1.0.0-1") < v("1.0.0-rc"), "数字排在字前面");
}

fn release(tag: &str, draft: bool, prerelease: bool) -> Value {
    json!({
        "tag_name": tag,
        "draft": draft,
        "prerelease": prerelease,
        "assets": [{"name": format!("{tag}.txt"), "browser_download_url": format!("https://x/{tag}.txt")}],
    })
}

#[test]
fn the_newest_release_skips_drafts_and_by_default_prereleases() {
    let list = json!([
        release("v1.0.0", false, false),
        release("v1.2.0", true, false),
        release("v1.1.0-rc.1", false, true),
        release("v1.0.1", false, false),
        release("nightly", false, false),
        release("v1.1.0-rc.2", false, false),
        release("v1.3.0", false, true),
    ]);
    let stable = newest(&list, false).unwrap();
    assert_eq!(
        stable.version,
        v("1.0.1"),
        "草稿、预发布（标的、标签里带的）都不算"
    );
    assert_eq!(
        stable.asset("v1.0.1.txt").unwrap().url,
        "https://x/v1.0.1.txt"
    );
    assert!(stable.asset("nope").is_none());
    assert_eq!(
        newest(&list, true).unwrap().version,
        v("1.3.0"),
        "标成预发布的，--prerelease 才算"
    );
    assert!(newest(&json!([]), false).is_none());
    assert!(newest(&json!({"message": "Not Found"}), false).is_none());
}

#[test]
fn the_archive_name_matches_dist() {
    assert_eq!(
        archive_name(&v("1.0.0"), "linux-x86_64"),
        "miyu-1.0.0-linux-x86_64.tar.gz"
    );
    assert_eq!(
        archive_name(&v("1.0.0-rc.1"), "windows-x86_64"),
        "miyu-1.0.0-rc.1-windows-x86_64.zip"
    );
}

#[test]
fn the_hash_is_found_by_exact_name() {
    let sums = "aaaa  miyu-1.0.0-linux-x86_64.tar.gz\nbbbb  miyu-1.0.0-linux-x86_64.tar.gz.old\n";
    assert_eq!(
        hash_of(sums, "miyu-1.0.0-linux-x86_64.tar.gz"),
        Some("aaaa".to_string())
    );
    assert_eq!(hash_of(sums, "miyu-1.0.0-macos-aarch64.tar.gz"), None);
}

#[test]
fn prereleases_are_noticed_for_the_hint() {
    assert!(has_pre(&json!([release("v1.0.0-rc.1", false, true)])));
    assert!(
        has_pre(&json!([release("v1.0.0", false, true)])),
        "标成预发布的"
    );
    assert!(
        has_pre(&json!([release("v1.0.0-rc.1", false, false)])),
        "标签带 - 的"
    );
    assert!(
        !has_pre(&json!([release("v1.0.0-rc.1", true, true)])),
        "草稿不算"
    );
    assert!(
        !has_pre(&json!([release("models-x", false, true)])),
        "标签不是 v<版本> 的不算"
    );
    assert!(!has_pre(&json!([release("v1.0.0", false, false)])));
    assert!(!has_pre(&json!({"message": "Not Found"})));
}

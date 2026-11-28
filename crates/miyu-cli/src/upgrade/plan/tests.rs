//! 标记、版本、订阅里的标签哪几个能算、压缩包名、清单里的那一行。

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

/// 年.月.序号（施工 VR-1，设计 59 第十节）：照数字比，跨月、跨年、十月以后不按字比，候选版排在同号正式版前面。
#[test]
fn year_month_versions_order_by_number() {
    let ordered = [
        "0.0.0",
        "2026.9.0",
        "2026.10.0",
        "2026.11.0-rc.1",
        "2026.11.0-rc.2",
        "2026.11.0-rc.10",
        "2026.11.0",
        "2026.11.1",
        "2026.11.10",
        "2026.12.0-rc.1",
        "2026.12.0",
        "2027.1.0",
    ];
    for pair in ordered.windows(2) {
        assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
    }
    let list = tags(&["v2026.11.0", "v2026.11.1", "v2026.12.0-rc.1", "v2026.9.3"]);
    assert_eq!(candidates(&list, false)[0].version, v("2026.11.1"));
    assert_eq!(candidates(&list, true)[0].version, v("2026.12.0-rc.1"));
}

fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|tag| (*tag).to_string()).collect()
}

/// 施工 UP-4：订阅里没有预发布的标记，照标签认，带 `-` 的是预发布；新的在前，标签原样留着。
#[test]
fn candidates_are_newest_first_and_prereleases_only_with_pre() {
    let list = tags(&[
        "v1.0.0",
        "v1.1.0-rc.1",
        "v1.0.1",
        "nightly",
        "1.2.0",
        "v1.1.0-rc.2",
        "v1.0.1",
    ]);
    let stable: Vec<String> = candidates(&list, false)
        .into_iter()
        .map(|release| release.tag)
        .collect();
    assert_eq!(
        stable,
        ["v1.0.1", "v1.0.0"],
        "预发布、认不出的、不带 v 的都不算，重的只算一次"
    );
    let all: Vec<String> = candidates(&list, true)
        .into_iter()
        .map(|release| release.tag)
        .collect();
    assert_eq!(all, ["v1.1.0-rc.2", "v1.1.0-rc.1", "v1.0.1", "v1.0.0"]);
    assert!(candidates(&[], true).is_empty());
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
    // 年.月.序号（施工 VR-2）：照版本原样拼，和 `cargo xtask dist` 打出来的同名。
    assert_eq!(
        archive_name(&v("2026.11.0-rc.1"), "linux-x86_64"),
        "miyu-2026.11.0-rc.1-linux-x86_64.tar.gz"
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
    assert!(has_pre(&tags(&["v1.0.0-rc.1"])), "标签带 - 的");
    assert!(
        !has_pre(&tags(&["models-x-rc.1", "1.0.0-rc.1"])),
        "标签不是 v<版本> 的不算"
    );
    assert!(!has_pre(&tags(&["v1.0.0"])));
    assert!(!has_pre(&[]));
}

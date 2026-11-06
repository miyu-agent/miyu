use super::{Version, required};
use crate::package::Code;

#[test]
fn three_numbers_and_nothing_else() {
    assert_eq!(Version::parse("0.2.0"), Some(Version([0, 2, 0])));
    assert_eq!(Version::parse("10.20.30"), Some(Version([10, 20, 30])));
    for wrong in [
        "1.2", "1.2.3.4", "a.b.c", "v1.2.3", "1..3", "1.2.-3", " 1.2.3", "+1.2.3", "",
    ] {
        assert_eq!(Version::parse(wrong), None, "{wrong:?}");
    }
}

#[test]
fn an_older_miyu_does_not_fit_and_a_development_build_always_does() {
    let wanted = Version([0, 2, 0]);
    assert!(!Version([0, 1, 9]).fits(wanted));
    assert!(Version([0, 2, 0]).fits(wanted), "一样的给得了");
    assert!(Version([1, 0, 0]).fits(wanted));
    assert!(
        Version([0, 0, 0]).fits(Version([99, 0, 0])),
        "开发时编的当最新"
    );
}

#[test]
fn a_newer_miyu_is_reported_before_anything_else() {
    let text = "[package]\nname = { en = \"N\" }\nmiyu = \"99.0.0\"\nfuture = true\n";
    let problem = required(text, "0.1.0").expect_err("要更新");
    assert_eq!(problem.code, Code::NeedsNewerMiyu);
    assert_eq!(problem.detail, "99.0.0");
    assert_eq!(problem.line, Some(3));
    assert!(required(text, "99.0.0").is_ok(), "一样的给得了");
    assert!(required(text, "0.0.0").is_ok(), "开发时编的不比");
    assert!(
        required("[package]\nname = { en = \"N\" }\n", "0.1.0").is_ok(),
        "没写的不比"
    );
    assert!(
        required("[package]\nmiyu = \"1.2\"\n", "0.1.0").is_ok(),
        "写法不对的交给读清单那一步报"
    );
}

/// 年.月.序号（施工 VR-1）：照数字比，十月以后不按字比；候选版（`-rc.N`）不是三段数字、不比，同开发版。
#[test]
fn a_year_month_floor_compares_by_number() {
    let text = "[package]\nname = { en = \"N\" }\nmiyu = \"2026.11.0\"\n";
    for (running, fits) in [
        ("2026.11.0", true),
        ("2026.11.1", true),
        ("2026.12.0", true),
        ("2027.1.0", true),
        ("2026.10.9", false),
        ("2026.9.0", false),
        ("2026.11.0-rc.1", true),
    ] {
        assert_eq!(required(text, running).is_ok(), fits, "{running}");
    }
}

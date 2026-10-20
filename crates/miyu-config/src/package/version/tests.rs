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

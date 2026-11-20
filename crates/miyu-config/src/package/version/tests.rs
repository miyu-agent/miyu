use super::{Semver, fits, maybe_newer, required};
use crate::package::{Code, read, read_for};

fn version(text: &str) -> Semver<'_> {
    Semver::parse(text).expect("合写法")
}

#[test]
fn semver_and_nothing_else() {
    for fine in [
        "0.2.0",
        "10.20.30",
        "2026.11.0",
        "2026.11.0-rc.2.4",
        "1.0.0+build.7",
    ] {
        assert!(Semver::parse(fine).is_some(), "{fine:?}");
    }
    for wrong in [
        "1.2",
        "1.2.3.4",
        "a.b.c",
        "v1.2.3",
        "1..3",
        "1.2.-3",
        " 1.2.3",
        "+1.2.3",
        "",
        "2026.11.01",
        "1.2.3-",
        "1.2.3-rc..1",
    ] {
        assert!(Semver::parse(wrong).is_none(), "{wrong:?}");
    }
}

#[test]
fn an_older_miyu_does_not_fit_and_a_development_build_always_does() {
    let wanted = version("0.2.0");
    assert!(!fits(&version("0.1.9"), &wanted));
    assert!(fits(&version("0.2.0"), &wanted), "一样的给得了");
    assert!(fits(&version("1.0.0"), &wanted));
    assert!(
        fits(&version("0.0.0"), &version("99.0.0")),
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

/// 年.月.序号（施工 VR-1）：照数字比，十月以后不按字比。候选版施工 VER-1 起照 semver 比：比同号的正式版低。
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
        ("2026.11.0-rc.1", false),
        ("2026.12.0-rc.1", true),
    ] {
        assert_eq!(required(text, running).is_ok(), fits, "{running}");
    }
}

/// 实测（施工 VER-1）：清单写 `miyu = "2026.11.0-rc.2.4"` 和一张新表，rc.2.3 的核心报了「不认识的表」，没先报要更新。
/// 候选版照 semver 一段一段比；写法查得过候选版的要求。
#[test]
fn a_candidate_build_compares_with_a_candidate_requirement() {
    let text = "[package]\nname = { en = \"N\" }\nversion = \"0.1.0\"\nmiyu = \"2026.11.0-rc.2.4\"\n\n[providers]\nx = 1\n";
    let problem = read_for(text, "2026.11.0-rc.2.3").expect_err("要更新");
    assert_eq!(problem.code, Code::NeedsNewerMiyu, "{problem}");
    assert_eq!(problem.detail, "2026.11.0-rc.2.4");
    assert_eq!(problem.line, Some(4));
    for (running, fits) in [
        ("2026.11.0-rc.2.3", false),
        ("2026.11.0-rc.2", false),
        ("2026.11.0-rc.1.9", false),
        ("2026.11.0-rc.2.4", true),
        ("2026.11.0-rc.2.10", true),
        ("2026.11.0-rc.3", true),
        ("2026.11.0", true),
        ("0.0.0", true),
    ] {
        assert_eq!(
            required(text, running).is_ok(),
            fits,
            "{running} 跑要 rc.2.4 的"
        );
    }
    let fine = "[package]\nname = { en = \"N\" }\nmiyu = \"2026.11.0-rc.2.4\"\n";
    assert!(read(fine).is_ok(), "候选版的要求写法对");
}

/// 不认识的表、键，清单又写了 `[package] miyu` 的，可能是这一版 Miyu 太旧（施工 VER-1）；别的出错、没写的不算。
#[test]
fn an_unknown_table_hints_at_a_newer_miyu_only_when_the_manifest_names_one() {
    let newer = "[package]\nname = { en = \"N\" }\nmiyu = \"2026.11.0\"\n\n[future]\nx = 1\n";
    let problem = read_for(newer, "0.0.0").expect_err("不认识");
    assert_eq!(problem.code, Code::UnknownTable);
    assert!(maybe_newer(newer, &problem));
    let key = "[package]\nname = { en = \"N\" }\nmiyu = \"2026.11.0\"\nfuture = true\n";
    let problem = read_for(key, "0.0.0").expect_err("不认识");
    assert_eq!(problem.code, Code::UnknownKey);
    assert!(maybe_newer(key, &problem));
    let unnamed = "[package]\nname = { en = \"N\" }\n\n[future]\nx = 1\n";
    let problem = read_for(unnamed, "0.0.0").expect_err("不认识");
    assert!(!maybe_newer(unnamed, &problem), "没写 miyu 的不提");
    let wrong = "[package]\nname = { en = \"N\" }\nmiyu = \"2026.11.0\"\nprotocol = \"x\"\n";
    let problem = read_for(wrong, "0.0.0").expect_err("写错");
    assert!(!maybe_newer(wrong, &problem), "写错的不提：{problem}");
}

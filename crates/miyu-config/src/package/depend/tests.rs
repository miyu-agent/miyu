//! 依赖的一项（施工 PK-2）：写法、照 semver 比。

use super::{Dependency, Semver};

#[test]
fn a_bare_id_has_no_floor_and_fits_anything_installed() {
    let dep = Dependency::parse("onebot").expect("读得成");
    assert_eq!(dep.id, "onebot");
    assert_eq!(dep.at_least, None);
    assert_eq!(dep.to_string(), "onebot");
    for version in [None, Some("r53"), Some("0.0.1")] {
        assert!(dep.fits(version), "{version:?}");
    }
}

#[test]
fn a_floor_is_written_after_greater_or_equal() {
    let dep = Dependency::parse("onebot>=1.2.0").expect("读得成");
    assert_eq!(dep.id, "onebot");
    assert_eq!(dep.at_least.as_deref(), Some("1.2.0"));
    assert_eq!(dep.to_string(), "onebot>=1.2.0", "照写的原样");
    assert_eq!(
        serde_json::to_value(&dep).unwrap(),
        serde_json::json!("onebot>=1.2.0")
    );
    let dep = Dependency::parse("a_b-2>=0.0.0-rc.1+build.5").expect("预发布、构建信息都认");
    assert_eq!(dep.at_least.as_deref(), Some("0.0.0-rc.1+build.5"));
}

/// 只认 `>=`、版本照 semver：别的写法读不成（读清单的报 `bad_dependency`）。
#[test]
fn other_spellings_are_refused() {
    for bad in [
        "",
        "Onebot",
        "onebot>=",
        "onebot>=1.2",
        "onebot>=1",
        "onebot>=v1.2.0",
        "onebot>=1.2.0.0",
        "onebot>=01.2.0",
        "onebot>=1.2.0-",
        "onebot>=1.2.0-rc..1",
        "onebot>=1.2.0-01",
        "onebot>=1.2.0+",
        "onebot>=1.2.0+a_b",
        "onebot >= 1.2.0",
        "onebot>= 1.2.0",
        "onebot>=1.2.0>=2.0.0",
        "onebot>1.2.0",
        "onebot=1.2.0",
        "onebot==1.2.0",
        "onebot<=1.2.0",
        "onebot~1.2.0",
        ">=1.2.0",
        "onebot>=99999999999999999999.0.0",
    ] {
        assert_eq!(Dependency::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn installed_versions_are_compared_by_semver() {
    let dep = Dependency::parse("x>=1.2.0").unwrap();
    for (version, fits) in [
        ("1.2.0", true),
        ("1.2.1", true),
        ("1.10.0", true),
        ("2.0.0", true),
        ("1.2.0+build", true),
        ("1.1.9", false),
        ("1.2.0-rc.1", false),
        ("0.9.0", false),
        ("1.2", false),
        ("r53", false),
        ("", false),
    ] {
        assert_eq!(dep.fits(Some(version)), fits, "{version}");
    }
    assert!(!dep.fits(None), "没写版本的不够");
}

/// semver 2.0.0 第 11 条的那一串：一个比一个高。
#[test]
fn precedence_follows_the_semver_spec() {
    let ordered = [
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-alpha.beta",
        "1.0.0-beta",
        "1.0.0-beta.2",
        "1.0.0-beta.11",
        "1.0.0-rc.1",
        "1.0.0",
        "1.0.1",
        "1.1.0",
        "2.0.0",
        "10.0.0",
    ];
    for pair in ordered.windows(2) {
        let (low, high) = (
            Semver::parse(pair[0]).unwrap(),
            Semver::parse(pair[1]).unwrap(),
        );
        assert!(low < high && high > low, "{} < {}", pair[0], pair[1]);
        let dep = Dependency::parse(&format!("x>={}", pair[1])).unwrap();
        assert!(!dep.fits(Some(pair[0])), "{} 不够 {}", pair[0], pair[1]);
        assert!(dep.fits(Some(pair[1])));
        let floor = Dependency::parse(&format!("x>={}", pair[0])).unwrap();
        assert!(floor.fits(Some(pair[1])), "{} 够 {}", pair[1], pair[0]);
    }
    assert_eq!(
        Semver::parse("1.0.0+a")
            .unwrap()
            .cmp(&Semver::parse("1.0.0+b").unwrap()),
        std::cmp::Ordering::Equal,
        "构建信息不算"
    );
}

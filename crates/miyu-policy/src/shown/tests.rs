//! 预设关了的功能照编号关掉那一块；没有预设的、没记这几个的都开着。

use super::*;

fn pin(off: &[&str]) -> PresetPin {
    PresetPin {
        id: "group".to_string(),
        off: off.iter().map(|id| (*id).to_string()).collect(),
        digest: None,
        absent: Vec::new(),
    }
}

#[test]
fn every_switch_is_read_from_the_preset() {
    assert_eq!(Shown::of(None), Shown::default());
    assert_eq!(
        Shown::of(Some(&pin(&["memory", "shell"]))),
        Shown::default()
    );
    let all = pin(&[TIME, WORKDIR, PERMISSION, SESSION, MODEL, HARNESS, MACHINE]);
    let shown = Shown::of(Some(&all));
    assert_eq!(
        shown.facts,
        Gates {
            time: false,
            workdir: false,
            permission: false,
            session: false,
            model: false,
        }
    );
    assert!(!shown.harness && !shown.machine);
    let one = Shown::of(Some(&pin(&[WORKDIR])));
    assert!(!one.facts.workdir);
    assert!(one.facts.time && one.facts.permission && one.harness && one.machine);
}

/// 装着并且没被关的才开（施工 ENV-1）：`env` 包整个关掉、卸掉（八个都记在没装的里）和八个都记在没开的里一模一样；只关
/// 「用户身份」的只有「关于你」不拼；以前的快照（没记没装的）照旧都开着。
#[test]
fn absent_switches_are_off_like_switched_off_ones() {
    let absent = PresetPin {
        absent: ALL.iter().map(|id| (*id).to_string()).collect(),
        ..pin(&[])
    };
    let all_off = pin(&ALL);
    assert_eq!(
        Shown::of(Some(&absent)),
        Shown::of(Some(&all_off)),
        "整包关和八项都关一样"
    );
    let off = Shown::of(Some(&absent));
    assert!(
        !off.facts.time && !off.harness && !off.machine && !off.profile,
        "{off:?}"
    );
    let profile = Shown::of(Some(&pin(&[PROFILE])));
    assert!(!profile.profile, "只关用户身份");
    assert!(
        profile.facts.time && profile.harness && profile.machine,
        "别的照旧"
    );
    assert!(Shown::of(Some(&pin(&[]))).profile, "以前的快照照旧开着");
}

/// 没装的照装着的功能表算（施工 ENV-1）：表里没有的那几个，照 `ALL` 的先后。
#[test]
fn absent_ones_are_those_not_installed() {
    let feature = |id: &str| crate::features::Feature {
        id: id.to_string(),
        package: PACKAGE.to_string(),
        tools: Vec::new(),
        service: None,
        script: None,
    };
    let some = Features::new(vec![feature(TIME), feature(PROFILE)]);
    assert_eq!(
        absent(&some),
        [WORKDIR, PERMISSION, SESSION, MODEL, HARNESS, MACHINE]
    );
    let all = Features::new(ALL.iter().map(|id| feature(id)).collect());
    assert!(absent(&all).is_empty(), "都装着的不记");
    assert_eq!(
        absent(&Features::new(Vec::new())).len(),
        ALL.len(),
        "整包没装"
    );
}

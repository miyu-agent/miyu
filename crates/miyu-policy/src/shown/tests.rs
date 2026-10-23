//! 预设关了的功能照编号关掉那一块；没有预设的、没记这几个的都开着。

use super::*;

fn pin(off: &[&str]) -> PresetPin {
    PresetPin {
        id: "group".to_string(),
        off: off.iter().map(|id| (*id).to_string()).collect(),
        digest: None,
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

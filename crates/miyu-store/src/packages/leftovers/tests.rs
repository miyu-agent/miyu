//! 卸了的包留下的改动（施工 P-11 下）：记、划掉、写回读出来一样，空的不留文件，读坏了的当没有。

use super::*;
use crate::test_support::Scratch;

fn ids(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

#[test]
fn leftovers_are_noted_cleared_and_kept_on_disk() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("state/packages");
    assert_eq!(Leftovers::read(&dir), Leftovers::default(), "没有的是空的");
    let mut left = Leftovers::default();
    left.left_by("kitty", &ids(&["cat"]), &ids(&["cozy"]));
    left.left_by("dog", &ids(&["rex"]), &[]);
    assert!(left.persona_of("cat", "kitty"));
    assert!(!left.persona_of("cat", "dog"));
    assert!(left.preset_of("cozy", "kitty"));
    left.write(&dir).unwrap();
    assert_eq!(Leftovers::read(&dir), left, "写回读出来一样");
    left.back("kitty");
    assert!(!left.persona_of("cat", "kitty"));
    assert!(left.persona_of("rex", "dog"), "别的包的不动");
    left.back("dog");
    left.write(&dir).unwrap();
    assert!(!dir.join(FILE).exists(), "空的不留文件");
    std::fs::write(dir.join(FILE), "not json").unwrap();
    assert_eq!(
        Leftovers::read(&dir),
        Leftovers::default(),
        "读坏了的当没有"
    );
}

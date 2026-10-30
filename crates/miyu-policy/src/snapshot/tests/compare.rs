//! 直接比两份快照和比字节（施工 V-24补）：换快照时判「和原来一样」直接比结构，不再各写一遍 JSON、各算一遍哈希。

use super::*;

/// 直接比两份快照和比字节（哈希）一回事（施工 V-24补：换快照时直接比，不再各写一遍、各算一遍）：一样的、读回来的相等；改了
/// 字、步数（换预设时照它重算）、续轮次数、有没有人看着、人格的，两样都不等。
#[test]
fn comparing_snapshots_agrees_with_comparing_their_bytes() {
    let one = engineer();
    let same = |a: &Snapshot, b: &Snapshot| {
        assert_eq!(a == b, a.hash() == b.hash(), "直接比和比字节说法一样");
        a == b
    };
    assert!(same(&one, &engineer()));
    assert!(same(&one, &Snapshot::from_bytes(&one.to_bytes()).unwrap()));
    let changed: [&dyn Fn(&mut Snapshot); 6] = [
        &|s| s.system.push('!'),
        &|s| s.step_limit = Some(7),
        &|s| s.auto_step_limit = Some(9),
        &|s| s.resumes += 1,
        &|s| s.attended = !s.attended,
        &|s| s.persona = None,
    ];
    for change in changed {
        let mut other = one.clone();
        change(&mut other);
        assert!(!same(&one, &other));
    }
}

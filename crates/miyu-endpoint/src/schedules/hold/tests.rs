//! 写不进的先放一放（施工 J-1 上）：头一次 1 秒、翻倍、最多 5 分钟；写进了清掉；没放着的照旧。

use super::*;

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("在时刻的范围里")
}

#[test]
fn it_backs_off_doubling_up_to_five_minutes_and_clears() {
    let holds = Holds::default();
    let alice = AccountId::parse("alice").expect("账号合写法");
    assert_eq!(
        holds.later(&alice, "t1", at(5_000)),
        at(5_000),
        "没放着的照旧"
    );
    assert_eq!(holds.failed(&alice, "t1", at(10_000)), at(11_000));
    assert_eq!(holds.failed(&alice, "t1", at(11_000)), at(13_000));
    assert_eq!(holds.failed(&alice, "t1", at(13_000)), at(17_000));
    assert_eq!(
        holds.later(&alice, "t1", at(5_000)),
        at(17_000),
        "照放着的往后挪"
    );
    assert_eq!(
        holds.later(&alice, "t1", at(20_000)),
        at(20_000),
        "本来就更晚的照旧"
    );
    assert_eq!(
        holds.later(&alice, "t2", at(5_000)),
        at(5_000),
        "别的不受连累"
    );
    for _ in 0..20 {
        holds.failed(&alice, "t1", at(0));
    }
    assert_eq!(
        holds.failed(&alice, "t1", at(0)),
        at(300_000),
        "最多 5 分钟"
    );
    holds.cleared(&alice, "t1");
    assert_eq!(
        holds.later(&alice, "t1", at(5_000)),
        at(5_000),
        "写进了清掉"
    );
    assert_eq!(
        holds.failed(&alice, "t1", at(0)),
        at(1_000),
        "再写不进从头算"
    );
}

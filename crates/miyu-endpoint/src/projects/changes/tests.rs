//! 项目变了的广播（施工 H-1）：一条带着账号，订阅的连接只认自己账号的。

use super::*;

#[test]
fn a_change_reaches_only_its_own_account() {
    let admin = AccountId::parse("admin").expect("合写法");
    let alice = AccountId::parse("alice").expect("合写法");
    let changes = Changes::default();
    let mut heard = changes.subscribe();
    changes.notify(&admin, "p1");
    let changed = heard.try_recv().expect("报了一声");
    assert_eq!(mine(&changed, &admin), Some("p1"));
    assert_eq!(mine(&changed, &alice), None, "别的账号的不推");
    assert_eq!(mine("no-newline", &admin), None);
}

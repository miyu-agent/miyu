//! 交接票（接入通讯平台要的）：一张只换一次、换出它绑的账号；过期的不认；最多 16 张，多了丢最早的。

use super::*;

fn alice() -> AccountId {
    AccountId::parse("alice").expect("合写法")
}

#[test]
fn a_ticket_is_taken_once_for_its_account() {
    let tickets = Tickets::default();
    let later = Instant::now() + TICKET_TTL;
    tickets.put("t1".to_string(), alice(), later);
    assert_eq!(tickets.take("t2"), None);
    assert_eq!(tickets.take("t1"), Some(alice()));
    assert_eq!(tickets.take("t1"), None, "只换一次");
}

#[test]
fn expired_tickets_are_refused_and_only_sixteen_are_kept() {
    let tickets = Tickets::default();
    tickets.put("old".to_string(), alice(), Instant::now());
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(tickets.take("old"), None, "过期的不认");
    let later = Instant::now() + TICKET_TTL;
    for n in 0..17 {
        tickets.put(format!("t{n}"), alice(), later);
    }
    assert_eq!(tickets.take("t0"), None, "多了丢最早的");
    assert_eq!(tickets.take("t1"), Some(alice()));
    assert_eq!(tickets.take("t16"), Some(alice()));
}
